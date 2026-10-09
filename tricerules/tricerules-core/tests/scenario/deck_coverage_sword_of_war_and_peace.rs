//! Exact Sword: paid equip, protection layers, real combat, actors and resolution-time counts.
use super::helpers::*;
use prost::Message;
use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::{Color, ContinuousEffectKind, ControllerReference, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const CARD: &str = "sword_of_war_and_peace";

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[10, 20, 30],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    e.enable_dev_commands();
    e
}

fn finish(e: &mut GameEngine) {
    for _ in 0..32 {
        answer_trigger_order_in_engine_order(e);
        if e.state.stack.is_empty() && e.state.blocking_choice().is_none() {
            return;
        }
        assert!(e.state.blocking_choice().is_none(), "unexpected choice");
        pass_priority_round(e);
    }
    panic!("Sword resolution exhausted");
}

fn source(e: &mut GameEngine) -> u32 {
    let id = inject_card_into_hand(e, 0, CARD);
    give_mana(
        e,
        10,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, CARD);
    semantic::accepted(e, 10, &cast_spell(slot, vec![]));
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    finish(e);
    assert_eq!(e.state.objects[&id].zone, Zone::Battlefield);
    id
}

fn equip(e: &mut GameEngine, sword: u32, host: u32) {
    give_mana(
        e,
        10,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(e, 10, sword, 0, target_object(host)).unwrap();
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    finish(e);
    assert_eq!(
        e.state.objects[&sword].attached_to,
        Some(AttachmentRecipient::Object(host))
    );
}

fn effect(e: &mut GameEngine, id: u32, kind: ContinuousEffectKind, timestamp: u64) {
    e.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(id),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp,
        trigger_grant_origin: None,
    });
}

fn move_sword(zone: DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 10,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: "Sword of War and Peace".into(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    }
}

fn protected(e: &GameEngine, host: u32) {
    let c = e.characteristics(host).unwrap();
    assert_eq!((c.power, c.toughness), (Some(4), Some(4)));
    assert_eq!(
        c.protections,
        [
            ProtectionQuality::Color(Color::Red),
            ProtectionQuality::Color(Color::White)
        ]
    );
}

fn attack(e: &mut GameEngine, host: u32, defender: i32) {
    semantic::accepted(e, 10, &primitive_yield());
    pass_priority_round(e);
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
    let legal = e.initial_response_batch().legal_by_player[&10]
        .legal_attack_assignments
        .clone();
    let assignment = legal
        .into_iter()
        .find(|a| a.attacker_object_id == host && a.defending_player_id == defender)
        .expect("legal actual defender assignment");
    semantic::accepted(
        e,
        10,
        &RuledCommand {
            cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                assignments: vec![assignment],
            })),
        },
    );
    pass_priority_round(e);
    assert_eq!(e.state.turn_step, TurnStep::DeclareBlockers);
}

fn hit(e: &mut GameEngine, host: u32, defender: i32) {
    attack(e, host, defender);
    if !e.state.combat.as_ref().unwrap().blockers_declared {
        semantic::accepted(e, defender, &declare_blockers(vec![]));
    }
    pass_priority_round(e);
    assert_eq!(e.state.stack.len(), 1, "one Sword combat trigger");
    assert_eq!(
        e.state.stack[0].trigger_context.affected_player,
        Some(defender)
    );
    assert_eq!(
        e.state.stack[0]
            .trigger_context
            .observed_object
            .unwrap()
            .object_id,
        host
    );
    assert!(
        e.state.stack[0].targets.is_empty(),
        "targetless Sword trigger"
    );
}

#[test]
fn sword_paid_cast_equip_reequip_and_departure_move_all_grants() {
    let mut e = setup(49901);
    let sword = source(&mut e);
    let a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(e.characteristics(a).unwrap().protections.is_empty());
    equip(&mut e, sword, a);
    protected(&e, a);
    assert!(e.characteristics(sword).unwrap().protections.is_empty());
    equip(&mut e, sword, b);
    protected(&e, b);
    assert_eq!(e.characteristics(a).unwrap().power, Some(2));
    assert!(e.characteristics(a).unwrap().protections.is_empty());
    semantic::accepted(&mut e, 10, &move_sword(DevZone::Graveyard));
    assert_eq!(e.characteristics(b).unwrap().power, Some(2));
    assert!(e.characteristics(b).unwrap().protections.is_empty());
}

#[test]
fn sword_equip_rejections_preserve_mana_stack_and_attachment() {
    let mut e = setup(49902);
    let sword = source(&mut e);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        10,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let cmd = activate_ability_for(&e, sword, 0, target_object(own));
    let before = e.diagnostic_snapshot().unwrap();
    assert!(e.apply_command(10, &cmd).is_err());
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    give_mana(
        &mut e,
        10,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    for (actor, target) in [(20, own), (10, other)] {
        let cmd = activate_ability_for(&e, sword, 0, target_object(target));
        let before = e.diagnostic_snapshot().unwrap();
        assert!(e.apply_command(actor, &cmd).is_err());
        assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    }
    semantic::accepted(&mut e, 10, &primitive_yield());
    let cmd = activate_ability_for(&e, sword, 0, target_object(own));
    let before = e.diagnostic_snapshot().unwrap();
    assert!(e.apply_command(10, &cmd).is_err());
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn sword_recipient_ability_removal_keeps_pt_and_later_equip_restores_protection() {
    let mut e = setup(49903);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    let stamp = e.state.command_index;
    effect(
        &mut e,
        host,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
        stamp,
    );
    assert_eq!(e.characteristics(host).unwrap().power, Some(4));
    assert!(e.characteristics(host).unwrap().protections.is_empty());
    // A new attachment supplies a later static-effect timestamp.
    let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, other);
    equip(&mut e, sword, host);
    protected(&e, host);
}

#[test]
fn sword_source_ability_removal_suppresses_grants_and_combat_trigger() {
    let mut e = setup(49904);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    let stamp = e.state.command_index + 1;
    effect(
        &mut e,
        sword,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
        stamp,
    );
    assert_eq!(e.characteristics(host).unwrap().power, Some(2));
    assert!(e.characteristics(host).unwrap().protections.is_empty());
    attack(&mut e, host, 20);
    if !e.state.combat.as_ref().unwrap().blockers_declared {
        semantic::accepted(&mut e, 20, &declare_blockers(vec![]));
    }
    pass_priority_round(&mut e);
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.players[1].life, 18);
}

#[test]
fn sword_red_white_targets_rejected_and_nontargeted_red_damage_prevented() {
    let mut e = setup(49905);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    for card in ["lightning_bolt", "swords_to_plowshares"] {
        inject_card_into_hand(&mut e, 0, card);
        give_mana(
            &mut e,
            10,
            ManaGift {
                r: 1,
                w: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, card);
        let before = e.diagnostic_snapshot().unwrap();
        assert!(e
            .apply_command(10, &cast_spell(slot, target_object(host)))
            .is_err());
        assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    }
    inject_card_into_hand(&mut e, 0, "pyroclasm");
    give_mana(
        &mut e,
        10,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "pyroclasm");
    semantic::accepted(&mut e, 10, &cast_spell(slot, vec![]));
    finish(&mut e);
    assert_eq!(e.state.objects[&host].damage, 0);
    protected(&e, host);
}

#[test]
fn sword_red_white_blocks_rejected_but_green_block_is_legal() {
    let mut e = setup(49906);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    let red = inject_creature_on_battlefield(&mut e, 1, "raging_goblin");
    let white = inject_creature_on_battlefield(&mut e, 1, "white_knight");
    let green = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    attack(&mut e, host, 20);
    for blocker in [red, white] {
        let before = e.diagnostic_snapshot().unwrap();
        assert!(e
            .apply_command(
                20,
                &declare_blockers(vec![BlockPair {
                    attacker_id: host,
                    blocker_id: blocker
                }])
            )
            .is_err());
        assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    }
    semantic::accepted(
        &mut e,
        20,
        &declare_blockers(vec![BlockPair {
            attacker_id: host,
            blocker_id: green,
        }]),
    );
    pass_priority_round(&mut e);
    assert!(
        e.state.stack.is_empty(),
        "damage to a creature does not trigger Sword"
    );
}

#[test]
fn sword_turning_red_detaches_and_host_death_detaches() {
    for red in [true, false] {
        let mut e = setup(49907 + u64::from(red));
        let sword = source(&mut e);
        let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, sword, host);
        if red {
            let stamp = e.state.command_index;
            effect(
                &mut e,
                sword,
                ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
                stamp,
            )
        } else {
            e.state.objects.get_mut(&host).unwrap().damage = 4;
        }
        semantic::accepted(&mut e, 10, &pass());
        assert!(e.state.objects[&sword].attached_to.is_none());
        assert_eq!(e.state.objects[&sword].zone, Zone::Battlefield);
        if red {
            assert_eq!(e.characteristics(host).unwrap().power, Some(2));
            assert!(e.characteristics(host).unwrap().protections.is_empty());
        } else {
            assert_eq!(e.state.objects[&host].zone, Zone::Graveyard);
        }
    }
}

#[test]
fn sword_counts_actual_damaged_player_and_controller_hands_at_resolution() {
    let mut e = setup(49910);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    hit(&mut e, host, 30);
    assert_eq!(e.state.players[2].life, 16);
    inject_card_into_hand(&mut e, 0, "forest");
    inject_card_into_hand(&mut e, 0, "forest");
    for _ in 0..4 {
        inject_card_into_hand(&mut e, 2, "forest");
    }
    let own = e.state.players[0].hand.len() as i32;
    let victim = e.state.players[2].hand.len() as i32;
    assert_ne!(own, victim);
    assert_ne!(victim, e.state.players[1].hand.len() as i32);
    finish(&mut e);
    assert_eq!(e.state.players[0].life, 20 + own);
    assert_eq!(e.state.players[1].life, 20);
    assert_eq!(e.state.players[2].life, 16 - victim);
    assert_eq!(e.state.players[0].hand.len() as i32, own);
    assert_eq!(e.state.players[2].hand.len() as i32, victim);
}

#[test]
fn sword_damage_lifelink_uses_current_controller_but_explicit_gain_uses_trigger_controller() {
    for leave_and_reenter in [false, true] {
        let mut e = setup(49911 + u64::from(leave_and_reenter));
        let sword = source(&mut e);
        let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, sword, host);
        let stamp = e.state.command_index;
        effect(
            &mut e,
            sword,
            ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
            stamp,
        );
        hit(&mut e, host, 30);
        assert_eq!(e.state.stack[0].controller, 10);
        let stamp = e.state.command_index;
        effect(
            &mut e,
            sword,
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(20),
            },
            stamp,
        );
        e.initial_response_batch();
        assert_eq!(e.characteristics(sword).unwrap().controller, 20);
        if leave_and_reenter {
            let generation = semantic::generation(&e, sword);
            semantic::accepted(&mut e, 10, &move_sword(DevZone::Graveyard));
            semantic::accepted(&mut e, 10, &move_sword(DevZone::Battlefield));
            assert_eq!(semantic::generation(&e, sword), generation + 2);
            assert_eq!(e.characteristics(sword).unwrap().controller, 10);
        }
        let own = e.state.players[0].hand.len() as i32;
        let victim = e.state.players[2].hand.len() as i32;
        finish(&mut e);
        assert_eq!(
            e.state.players[0].life,
            20 + own,
            "captured controller gains explicitly"
        );
        assert_eq!(
            e.state.players[1].life,
            20 + victim,
            "current or old-incarnation LKI controller gains lifelink"
        );
        assert_eq!(e.state.players[2].life, 16 - victim);
    }
}

#[test]
fn sword_zero_hand_or_prevented_trigger_damage_still_gains_life() {
    for empty_hand in [true, false] {
        let mut e = setup(49920 + u64::from(empty_hand));
        let sword = source(&mut e);
        let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, sword, host);
        hit(&mut e, host, 30);
        if empty_hand {
            let ids = std::mem::take(&mut e.state.players[2].hand);
            for id in ids {
                e.state.objects.get_mut(&id).unwrap().zone = Zone::Graveyard;
                e.state.players[2].graveyard.push(id);
            }
        } else {
            e.state.add_damage_prevention_shield(30, 100);
        }
        let own = e.state.players[0].hand.len() as i32;
        finish(&mut e);
        assert_eq!(e.state.players[0].life, 20 + own);
        assert_eq!(e.state.players[2].life, 16);
    }
}

#[test]
fn sword_prevented_combat_and_noncombat_host_damage_do_not_trigger() {
    let mut e = setup(49922);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    e.state.add_damage_prevention_shield(30, 4);
    attack(&mut e, host, 30);
    if !e.state.combat.as_ref().unwrap().blockers_declared {
        semantic::accepted(&mut e, 30, &declare_blockers(vec![]));
    }
    pass_priority_round(&mut e);
    assert_eq!(e.state.players[2].life, 20);
    assert!(e.state.stack.is_empty());
    let mut e = setup(49923);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "prodigal_pyromancer");
    equip(&mut e, sword, host);
    apply_ability(&mut e, 10, host, 0, target_player(30)).unwrap();
    pass_priority_round(&mut e);
    assert_eq!(e.state.players[2].life, 19);
    assert!(e.state.stack.is_empty());
}

#[test]
fn sword_host_control_differs_from_equipment_control() {
    let mut e = setup(49924);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    let stamp = e.state.command_index;
    effect(
        &mut e,
        sword,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(20),
        },
        stamp,
    );
    e.initial_response_batch();
    protected(&e, host);
    hit(&mut e, host, 30);
    assert_eq!(e.state.stack[0].controller, 20);
    let hand = e.state.players[1].hand.len() as i32;
    let victim = e.state.players[2].hand.len() as i32;
    finish(&mut e);
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20 + hand);
    assert_eq!(e.state.players[2].life, 16 - victim);
}

#[test]
fn sword_host_stops_being_creature_and_stale_equip_cannot_attach() {
    let mut e = setup(49925);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    let replacement = tricerules_cards::TypeLineReplacement {
        card_types: vec![tricerules_cards::PermanentTypeFilter::Artifact],
        creature_types: vec![],
        land_types: vec![],
    };
    let stamp = e.state.command_index;
    effect(
        &mut e,
        host,
        ContinuousEffectKind::Layer4SetTypeLine(replacement),
        stamp,
    );
    semantic::accepted(&mut e, 10, &pass());
    assert!(e.state.objects[&sword].attached_to.is_none());
    let mut e = setup(49926);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        10,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 10, sword, 0, target_object(host)).unwrap();
    let movement = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 10,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: "Grizzly Bears".into(),
                zone: DevZone::Graveyard as i32,
                ready: false,
            })),
        })),
    };
    semantic::accepted(&mut e, 10, &movement);
    finish(&mut e);
    assert!(e.state.objects[&sword].attached_to.is_none());
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn sword_combat_damage_to_planeswalker_does_not_trigger() {
    let mut e = setup(49927);
    let sword = source(&mut e);
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, sword, host);
    let walker = inject_permanent_on_battlefield(&mut e, 2, "jace_beleren");
    e.state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(tricerules_cards::CounterKind::Loyalty, 8);
    semantic::accepted(&mut e, 10, &primitive_yield());
    pass_priority_round(&mut e);
    let assignment = *e.initial_response_batch().legal_by_player[&10]
        .legal_attack_assignments
        .iter()
        .find(|a| {
            a.attacker_object_id == host
                && a.defender.as_ref().is_some_and(|d| d.object_id == walker)
        })
        .expect("actual planeswalker defender");
    semantic::accepted(
        &mut e,
        10,
        &RuledCommand {
            cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                assignments: vec![assignment],
            })),
        },
    );
    pass_priority_round(&mut e);
    assert!(e.state.combat.as_ref().unwrap().blockers_declared);
    pass_priority_round(&mut e);
    assert_eq!(
        e.state.objects[&walker].counter_count(tricerules_cards::CounterKind::Loyalty),
        4
    );
    assert_eq!(e.state.players[2].life, 20);
    assert!(e.state.stack.is_empty());
}

#[test]
fn sword_paid_cast_equip_combat_and_trigger_commands_replay_identically() {
    fn fresh() -> (GameEngine, u32, u32) {
        let mut e = setup(49930);
        let sword = inject_card_into_hand(&mut e, 0, CARD);
        let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            10,
            ManaGift {
                c: 5,
                ..Default::default()
            },
        );
        (e, sword, host)
    }
    fn apply(
        e: &mut GameEngine,
        commands: &mut Vec<(i32, RuledCommand)>,
        events: &mut Vec<RuledEventBatch>,
        actor: i32,
        cmd: RuledCommand,
    ) {
        let bytes = cmd.encode_to_vec();
        let decoded = RuledCommand::decode(bytes.as_slice()).unwrap();
        events.push(semantic::accepted(e, actor, &decoded));
        commands.push((actor, decoded));
    }
    fn drain(
        e: &mut GameEngine,
        commands: &mut Vec<(i32, RuledCommand)>,
        events: &mut Vec<RuledEventBatch>,
    ) {
        for _ in 0..24 {
            if e.state.stack.is_empty() {
                return;
            }
            assert!(e.state.blocking_choice().is_none());
            let actor = e.state.priority_player_id();
            apply(e, commands, events, actor, pass());
        }
        panic!("replay drain")
    }
    let (mut e, sword, host) = fresh();
    let mut commands = vec![];
    let mut events = vec![];
    let slot = hand_index_for_card(&e, 0, CARD);
    apply(
        &mut e,
        &mut commands,
        &mut events,
        10,
        cast_spell(slot, vec![]),
    );
    drain(&mut e, &mut commands, &mut events);
    let cmd = activate_ability_for(&e, sword, 0, target_object(host));
    apply(&mut e, &mut commands, &mut events, 10, cmd);
    drain(&mut e, &mut commands, &mut events);
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    protected(&e, host);
    apply(&mut e, &mut commands, &mut events, 10, primitive_yield());
    for _ in 0..3 {
        let actor = e.state.priority_player_id();
        apply(&mut e, &mut commands, &mut events, actor, pass());
    }
    let assignment = *e.initial_response_batch().legal_by_player[&10]
        .legal_attack_assignments
        .iter()
        .find(|a| a.attacker_object_id == host && a.defending_player_id == 30)
        .unwrap();
    apply(
        &mut e,
        &mut commands,
        &mut events,
        10,
        RuledCommand {
            cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                assignments: vec![assignment],
            })),
        },
    );
    for _ in 0..3 {
        let actor = e.state.priority_player_id();
        apply(&mut e, &mut commands, &mut events, actor, pass());
    }
    if !e.state.combat.as_ref().unwrap().blockers_declared {
        apply(
            &mut e,
            &mut commands,
            &mut events,
            30,
            declare_blockers(vec![]),
        );
    }
    for _ in 0..3 {
        let actor = e.state.priority_player_id();
        apply(&mut e, &mut commands, &mut events, actor, pass());
    }
    assert_eq!(e.state.stack.len(), 1);
    let own = e.state.players[0].hand.len() as i32;
    let victim = e.state.players[2].hand.len() as i32;
    drain(&mut e, &mut commands, &mut events);
    assert_eq!(e.state.players[0].life, 20 + own);
    assert_eq!(e.state.players[2].life, 16 - victim);
    let (mut replay, _, _) = fresh();
    let replay_events: Vec<_> = commands
        .iter()
        .map(|(actor, cmd)| replay.apply_command(*actor, cmd).unwrap())
        .collect();
    assert_eq!(events, replay_events);
    assert_eq!(
        e.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
