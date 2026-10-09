//! Actual Colossus: derived mana-value sum, locked costs and exact graveyard-source payments.
use super::helpers::*;
use prost::Message;
use tricerules_cards::{
    ContinuousEffectKind, ControllerReference, EffectDuration, PermanentTypeFilter,
    TypeLineReplacement,
};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, AbilitySourceZone, BeginSpellCast, CancelSpellCast, CommitSpellCast,
    CostObjectRef, CostObjectRefs, DevCommand, DevMoveCard, DevZone, PaymentMana, PaymentSelection,
    PreviewPayment, SpellCastAnnouncement,
};

const CARD: &str = "metalwork_colossus";

fn setup() -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        26100472,
        &[10, 20, 30],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    inject_card_into_hand(&mut e, 0, CARD);
    e
}

fn rejected(e: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = e.diagnostic_snapshot().unwrap();
    assert!(e.apply_command(actor, command).is_err(), "{command:?}");
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
}

fn modify(e: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
    e.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(object),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: e.state.command_index,
    });
    e.initial_response_batch();
}

fn begin_command(e: &GameEngine) -> RuledCommand {
    let Some(Cmd::CastSpell(cast)) = cast_spell(hand_index_for_card(e, 0, CARD), vec![]).cmd else {
        unreachable!()
    };
    RuledCommand {
        cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
            announcement: Some(SpellCastAnnouncement {
                source: cast.source,
                cast_method: cast.cast_method,
                ..Default::default()
            }),
        })),
    }
}

fn expect_cost(e: &mut GameEngine, expected: &str) {
    let command = begin_command(e);
    e.apply_command(10, &command).unwrap();
    let pending = e.state.pending_spell_cast.as_ref().unwrap();
    assert_eq!(pending.locked_total_cost, expected);
    let transaction_id = pending.transaction_id;
    e.apply_command(
        10,
        &RuledCommand {
            cmd: Some(Cmd::CancelSpellCast(CancelSpellCast { transaction_id })),
        },
    )
    .unwrap();
}

fn activation(e: &GameEngine, source: u32, members: &[u32]) -> RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(a)) = command.cmd.as_mut() else {
        unreachable!()
    };
    a.source_zone = AbilitySourceZone::Graveyard as i32;
    a.cost_selections = vec![CostSelection {
        cost_index: 0,
        selection: Some(
            tricerules_proto::cost_selection::Selection::BattlefieldObjects(CostObjectRefs {
                objects: members
                    .iter()
                    .map(|id| CostObjectRef {
                        object_id: *id,
                        zone_change_generation: e
                            .state
                            .zone_change_generation
                            .get(id)
                            .copied()
                            .unwrap_or(0),
                    })
                    .collect(),
            }),
        ),
    }];
    command
}

fn resolve_one(e: &mut GameEngine) {
    let count = e.state.players.iter().filter(|p| !p.has_lost).count()
        - e.state.passes_since_stack_change as usize;
    for _ in 0..count {
        let actor = e.state.priority_player_id();
        e.apply_command(actor, &pass()).unwrap();
    }
}

#[test]
fn metalwork_sums_mana_values_excludes_creatures_opponents_and_floors_at_zero() {
    let mut e = setup();
    expect_cost(&mut e, "{11}");
    let ring = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    inject_permanent_on_battlefield(&mut e, 0, "krark-clan_ironworks");
    e.state.objects.get_mut(&ring).unwrap().tapped = true;
    inject_permanent_on_battlefield(&mut e, 0, "kuldotha_forgemaster");
    inject_permanent_on_battlefield(&mut e, 0, "unnatural_growth");
    inject_permanent_on_battlefield(&mut e, 1, "krark-clan_ironworks");
    inject_permanent_on_battlefield(&mut e, 2, "krark-clan_ironworks");
    expect_cost(&mut e, "{6}"); // 1 + 4, not two artifacts or five permanents.
    inject_permanent_on_battlefield(&mut e, 0, "hedron_archive");
    inject_permanent_on_battlefield(&mut e, 0, "mind_stone");
    expect_cost(&mut e, "{0}"); // exactly eleven.
    inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    expect_cost(&mut e, "{0}"); // above eleven.
}

#[test]
fn metalwork_uses_derived_types_control_copy_and_battlefield_x_mana_value() {
    let mut e = setup();
    let ring = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    let stolen = inject_permanent_on_battlefield(&mut e, 1, "krark-clan_ironworks");
    expect_cost(&mut e, "{10}");
    modify(
        &mut e,
        stolen,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(10),
        },
    );
    expect_cost(&mut e, "{6}");
    modify(
        &mut e,
        ring,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    expect_cost(&mut e, "{7}");
    let creature = inject_permanent_on_battlefield(&mut e, 0, "kuldotha_forgemaster");
    modify(
        &mut e,
        creature,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    expect_cost(&mut e, "{2}");
    modify(
        &mut e,
        stolen,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(30),
        },
    );
    expect_cost(&mut e, "{6}");

    let mut e = setup();
    let token = inject_creature_on_battlefield(&mut e, 0, "myr_token_c_1_1");
    // A token with no mana cost, made a noncreature artifact, contributes zero.
    modify(
        &mut e,
        token,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    expect_cost(&mut e, "{11}");
    let face = tricerules_cards::registry::global()
        .get("krark-clan_ironworks")
        .unwrap()
        .primary_face()
        .clone();
    e.state.objects.get_mut(&token).unwrap().copiable_values =
        Some(tricerules_core::state::CopiableValues {
            source_card_id: "krark-clan_ironworks".into(),
            source_face_index: 0,
            display_name: face.name.clone(),
            face,
            room_faces: None,
        });
    assert!(e.state.objects[&token].is_token());
    assert_eq!(e.characteristics(token).unwrap().mana_value, 4);
    expect_cost(&mut e, "{7}");
    let cornucopia = inject_permanent_on_battlefield(&mut e, 0, "astral_cornucopia");
    e.state
        .objects
        .get_mut(&cornucopia)
        .unwrap()
        .set_counter(tricerules_cards::CounterKind::Charge, 7);
    assert_eq!(e.characteristics(cornucopia).unwrap().mana_value, 0);
    expect_cost(&mut e, "{7}");
}

#[test]
fn metalwork_locked_real_ironworks_payment_paid_cast_and_serialized_replay() {
    fn fresh() -> (GameEngine, u32, u32) {
        let mut e = setup();
        let ironworks = inject_permanent_on_battlefield(&mut e, 0, "krark-clan_ironworks");
        let stone = inject_permanent_on_battlefield(&mut e, 0, "mind_stone");
        inject_graveyard_card(&mut e, 0, CARD);
        e.state.players[0].mana_pool.colorless = 3;
        (e, ironworks, stone)
    }
    let (mut e, ironworks, stone) = fresh();
    let source = e.state.players[0].hand[hand_index_for_card(&e, 0, CARD)];
    let mut commands = vec![];
    let mut events = vec![];
    let mut apply = |e: &mut GameEngine, actor: i32, command: RuledCommand| {
        let bytes = command.encode_to_vec();
        let decoded = RuledCommand::decode(bytes.as_slice()).unwrap();
        events.push(e.apply_command(actor, &decoded).unwrap());
        commands.push((actor, decoded));
    };
    let begin = begin_command(&e);
    rejected(&mut e, 20, &begin);
    let mut invalid_x = begin.clone();
    let Some(Cmd::BeginSpellCast(b)) = invalid_x.cmd.as_mut() else {
        unreachable!()
    };
    b.announcement.as_mut().unwrap().x_value = 1;
    rejected(&mut e, 10, &invalid_x);
    apply(&mut e, 10, begin);
    let pending = e.state.pending_spell_cast.as_ref().unwrap();
    assert_eq!(pending.locked_total_cost, "{5}");
    let transaction_id = pending.transaction_id;
    let insufficient = RuledCommand {
        cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
            transaction_id,
            payment: Some(PaymentSelection {
                expected_state_revision: e.state.command_index,
                source: Some(CostObjectRef {
                    object_id: source,
                    zone_change_generation: e
                        .state
                        .zone_change_generation
                        .get(&source)
                        .copied()
                        .unwrap_or(0),
                }),
                mana: Some(PaymentMana {
                    c: 3,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        })),
    };
    rejected(&mut e, 10, &insufficient);
    let gy_source = *e.state.players[0]
        .graveyard
        .iter()
        .find(|id| e.state.objects[id].card_id == CARD)
        .unwrap();
    let nonmana = activation(&e, gy_source, &[ironworks, stone]);
    rejected(&mut e, 10, &nonmana);
    let mut mana = activate_ability_for(&e, ironworks, 0, vec![]);
    let Some(Cmd::ActivateAbility(a)) = mana.cmd.as_mut() else {
        unreachable!()
    };
    a.cost_selections = vec![permanent_cost_selection(0, stone)];
    rejected(&mut e, 20, &mana);
    apply(&mut e, 10, mana);
    assert_eq!(e.state.objects[&stone].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].mana_pool.colorless, 5);
    assert_eq!(
        e.state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .locked_total_cost,
        "{5}"
    );
    let preview = e.preview_payment(
        10,
        &PreviewPayment {
            transaction_id,
            revision: 1,
            commit_spell_cast: Some(CommitSpellCast {
                transaction_id,
                payment: Some(PaymentSelection {
                    mana: Some(PaymentMana {
                        c: 5,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    let commit = RuledCommand {
        cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
            transaction_id,
            payment: preview.selection,
            restricted_mana: preview.restricted_mana,
        })),
    };
    apply(&mut e, 10, commit);
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    for _ in 0..3 {
        let actor = e.state.priority_player_id();
        apply(&mut e, actor, pass());
    }
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    let c = e.characteristics(source).unwrap();
    assert!(c.is_artifact() && c.is_creature() && c.colors.is_empty());
    assert_eq!(
        (c.power, c.toughness, c.mana_value),
        (Some(10), Some(10), 11)
    );
    let (mut replay, _, _) = fresh();
    let replayed: Vec<_> = commands
        .iter()
        .map(|(actor, command)| replay.apply_command(*actor, command).unwrap())
        .collect();
    assert_eq!(events, replayed);
    assert_eq!(
        e.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn metalwork_graveyard_exact_cost_offers_reject_invalid_atomic_and_return_only_source() {
    let mut e = setup();
    let source = inject_graveyard_card(&mut e, 0, CARD);
    let other = inject_graveyard_card(&mut e, 0, CARD);
    let a = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    let key = u64::from(source) << 32;
    let offer = e.initial_response_batch();
    assert!(!offer.legal_by_player[&10].cost_choices_by_ability[&key].non_mana_costs_payable);
    let b = inject_permanent_on_battlefield(&mut e, 0, "bottle_gnomes");
    e.state.objects.get_mut(&b).unwrap().tapped = true;
    e.state.objects.get_mut(&b).unwrap().owner = 30;
    let foreign = inject_permanent_on_battlefield(&mut e, 1, "sol_ring");
    let nonartifact = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let offer = e.initial_response_batch();
    let costs = &offer.legal_by_player[&10].cost_choices_by_ability[&key];
    let zone_offer = offer.legal_by_player[&10]
        .zone_ability_actions
        .iter()
        .find(|a| a.object_id == source)
        .unwrap();
    assert_eq!(zone_offer.source_zone, AbilitySourceZone::Graveyard as i32);
    assert_eq!(
        zone_offer.zone_change_generation,
        e.state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0)
    );
    let presentation = zone_offer
        .ability
        .as_ref()
        .unwrap()
        .presentation
        .as_ref()
        .unwrap();
    assert_eq!(presentation.card_id, CARD);
    assert_eq!(presentation.face_id, CARD);
    assert_eq!(presentation.path[0].id, "activated_01");
    assert_eq!(presentation.oracle_line_indices, [2]);
    assert!(!offer.legal_by_player[&20]
        .zone_ability_actions
        .iter()
        .any(|a| a.object_id == source));
    assert!(costs.non_mana_costs_payable);
    assert_eq!(costs.choices.len(), 1);
    let choice = &costs.choices[0];
    assert_eq!((choice.cost_index, choice.min, choice.max), (0, 2, 2));
    let candidates: Vec<_> = choice
        .candidate_objects
        .iter()
        .filter_map(|c| c.object.map(|o| o.object_id))
        .collect();
    assert!(candidates.contains(&a) && candidates.contains(&b));
    assert!(!candidates.contains(&foreign) && !candidates.contains(&nonartifact));
    for candidate in &choice.candidate_objects {
        let object = candidate.object.unwrap();
        assert_eq!(
            object.zone_change_generation,
            e.state
                .zone_change_generation
                .get(&object.object_id)
                .copied()
                .unwrap_or(0)
        );
    }
    for members in [
        vec![a],
        vec![a, b, foreign],
        vec![a, a],
        vec![a, foreign],
        vec![a, nonartifact],
    ] {
        let bad = activation(&e, source, &members);
        rejected(&mut e, 10, &bad);
    }
    let command = activation(&e, source, &[a, b]);
    rejected(&mut e, 20, &command);
    for variant in 0..7 {
        let mut bad = command.clone();
        let Some(Cmd::ActivateAbility(act)) = bad.cmd.as_mut() else {
            unreachable!()
        };
        match variant {
            0 => act.expected_zone_change_generation += 1,
            1 => {
                let Some(tricerules_proto::cost_selection::Selection::BattlefieldObjects(refs)) =
                    act.cost_selections[0].selection.as_mut()
                else {
                    unreachable!()
                };
                refs.objects[1].zone_change_generation += 1;
            }
            2 => act.cost_selections.push(act.cost_selections[0].clone()),
            3 => act.cost_selections[0] = permanent_cost_selection(0, a),
            4 => act.cost_selections[0].cost_index = 1,
            5 => act.source_zone = AbilitySourceZone::Battlefield as i32,
            6 => act.targets = target_object(a),
            _ => unreachable!(),
        }
        rejected(&mut e, 10, &bad);
    }
    let hand_source = e.state.players[0].hand[hand_index_for_card(&e, 0, CARD)];
    let wrong_zone = activation(&e, hand_source, &[a, b]);
    rejected(&mut e, 10, &wrong_zone);
    let generation = e
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    e.apply_command(10, &command).unwrap();
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&other].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&a));
    assert!(e.state.players[2].graveyard.contains(&b));
    assert_eq!(e.state.stack.len(), 1);
    resolve_one(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&source));
    assert_eq!(e.state.zone_change_generation[&source], generation + 1);
    assert_eq!(e.state.objects[&other].zone, Zone::Graveyard);
    rejected(&mut e, 10, &command);
}

fn move_source(e: &mut GameEngine, zone: DevZone) {
    e.enable_dev_commands();
    e.apply_command(
        10,
        &RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: 10,
                dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                    card_name: "Metalwork Colossus".into(),
                    zone: zone as i32,
                    ready: false,
                })),
            })),
        },
    )
    .unwrap();
}

#[test]
fn metalwork_departed_or_reentered_source_does_not_return_old_incarnation() {
    for reenter in [false, true] {
        let mut e = setup();
        // Remove the unrelated hand copy so the dev move binds the sole exact source.
        let hand = e.state.players[0].hand[hand_index_for_card(&e, 0, CARD)];
        e.state.players[0].hand.retain(|id| *id != hand);
        e.state.objects.remove(&hand);
        let source = inject_graveyard_card(&mut e, 0, CARD);
        let a = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
        let b = inject_permanent_on_battlefield(&mut e, 0, "mind_stone");
        let command = activation(&e, source, &[a, b]);
        e.apply_command(10, &command).unwrap();
        move_source(&mut e, DevZone::Exile);
        if reenter {
            move_source(&mut e, DevZone::Graveyard);
        }
        resolve_one(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            if reenter {
                Zone::Graveyard
            } else {
                Zone::Exile
            }
        );
        assert!(!e.state.players[0].hand.contains(&source));
    }
}

#[test]
fn metalwork_second_activation_and_serialized_return_replay_retrieve_only_once() {
    fn fresh() -> (GameEngine, u32, Vec<u32>) {
        let mut e = setup();
        let source = inject_graveyard_card(&mut e, 0, CARD);
        let members = (0..4)
            .map(|_| inject_permanent_on_battlefield(&mut e, 0, "sol_ring"))
            .collect();
        (e, source, members)
    }
    let (mut e, source, members) = fresh();
    let mut commands = vec![];
    let mut events = vec![];
    for pair in members.chunks(2) {
        let command = activation(&e, source, pair);
        let bytes = command.encode_to_vec();
        let decoded = RuledCommand::decode(bytes.as_slice()).unwrap();
        events.push(e.apply_command(10, &decoded).unwrap());
        commands.push((10, decoded));
    }
    assert_eq!(e.state.stack.len(), 2);
    for resolution in 0..2 {
        for _ in 0..3 {
            let actor = e.state.priority_player_id();
            events.push(e.apply_command(actor, &pass()).unwrap());
            commands.push((actor, pass()));
        }
        assert_eq!(e.state.objects[&source].zone, Zone::Hand);
        assert_eq!(e.state.stack.len(), 1 - resolution);
    }
    assert_eq!(
        e.state.players[0]
            .hand
            .iter()
            .filter(|id| **id == source)
            .count(),
        1
    );
    let (mut replay, _, _) = fresh();
    let replayed: Vec<_> = commands
        .iter()
        .map(|(actor, command)| replay.apply_command(*actor, command).unwrap())
        .collect();
    assert_eq!(events, replayed);
    assert_eq!(
        e.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn metalwork_simultaneous_cost_departing_observer_and_exile_replacement() {
    use tricerules_cards::primitives::{CastTriggerPlayer, TriggerCondition, ZoneEventCardinality};
    for replace in [false, true] {
        let mut e = setup();
        let source = inject_graveyard_card(&mut e, 0, CARD);
        let observer = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
        let member = inject_permanent_on_battlefield(&mut e, 0, "mind_stone");
        let mut ability = tricerules_cards::registry::global()
            .get("ajanis_pridemate")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.trigger = TriggerCondition::WheneverPermanentLeavesBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: Default::default(),
            destination: Default::default(),
            cardinality: ZoneEventCardinality::EachObject,
        };
        e.state.add_triggered_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(observer),
            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: e.state.command_index,
        });
        if replace {
            e.state.death_replacement_effects.push(
                tricerules_core::state::ActiveDeathReplacement {
                    object_id: member,
                    zone_change_generation: e
                        .state
                        .zone_change_generation
                        .get(&member)
                        .copied()
                        .unwrap_or(0),
                },
            );
        }
        let command = activation(&e, source, &[observer, member]);
        e.apply_command(10, &command).unwrap();
        let captured = e.state.pending_triggers.len()
            + e.state
                .pending_trigger_order
                .as_ref()
                .map_or(0, |p| p.candidates.len())
            + e.state.stack.len()
            - 1;
        assert_eq!(
            captured, 2,
            "departing observer sees both objects from one sacrifice group"
        );
        assert_eq!(e.state.objects[&observer].zone, Zone::Graveyard);
        assert_eq!(
            e.state.objects[&member].zone,
            if replace {
                Zone::Exile
            } else {
                Zone::Graveyard
            }
        );
        assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    }
}
