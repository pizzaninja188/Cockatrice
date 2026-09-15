//! Issue #298 — the four reviewed Aura ETB keyword grants use the shared attachment,
//! generation, temporary-effect, and continuous-modifier contracts.
//!
//! Oracle and current Comprehensive Rules were checked 2026-09-15. CR 303.4, 603.6,
//! 608.2b, 611.2a, 613.1f, 514.2, 704.5m, and 400.7 govern Aura attachment,
//! enters-the-battlefield triggers, attached-object resolution, temporary keyword expiry, static
//! attached modifiers, Aura state-based cleanup, and leave/re-entry generations.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

fn aura_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn three_player_main1(seed: u64) -> GameEngine {
    let mut engine = aura_engine(seed);
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    engine
}

fn resolve_entire_stack_three_player(engine: &mut GameEngine) {
    loop {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        for _ in 0..engine.state.players.len() {
            if engine.state.stack.is_empty() {
                break;
            }
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
        }
    }
}

fn cast_and_resolve_aura(
    engine: &mut GameEngine,
    card_id: &str,
    target: u32,
    mana: ManaGift,
) -> u32 {
    inject_card_into_hand(engine, 0, card_id);
    give_mana(engine, 0, mana);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
    resolve_entire_stack_two_player(engine);
    battlefield_object_for_card(engine, 0, card_id)
}

fn cast_aura_until_etb_trigger(
    engine: &mut GameEngine,
    card_id: &str,
    target: u32,
    mana: ManaGift,
) -> u32 {
    inject_card_into_hand(engine, 0, card_id);
    give_mana(engine, 0, mana);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
    engine.apply_command(0, &pass()).expect("caster pass");
    engine
        .apply_command(1, &pass())
        .expect("opponent pass resolves Aura spell");
    let aura = battlefield_object_for_card(engine, 0, card_id);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the Aura ETB trigger is pending"
    );
    aura
}

fn move_battlefield_object_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn return_object_to_battlefield(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].battlefield.push(object_id);
    let object = engine.state.objects.get_mut(&object_id).expect("object");
    object.zone = Zone::Battlefield;
    object.tapped = false;
    object.summoning_sick = false;
    object.attached_to = None;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn dev_move_card(engine: &mut GameEngine, player_id: i32, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move card through the engine dev path");
}

#[test]
fn issue_298_each_allowlisted_aura_grants_temporary_keyword_and_keeps_static_modifier() {
    let cases = [
        (
            "super_speed",
            ManaGift {
                r: 1,
                ..Default::default()
            },
            Keyword::FirstStrike,
            3,
            2,
            Some(Keyword::Haste),
        ),
        (
            "fire-rim_form",
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            Keyword::FirstStrike,
            4,
            2,
            None,
        ),
        (
            "aquitects_defenses",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            Keyword::Hexproof,
            3,
            4,
            None,
        ),
        (
            "fae_flight",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            Keyword::Hexproof,
            3,
            2,
            Some(Keyword::Flying),
        ),
    ];

    for (offset, (card_id, mana, temporary, power, toughness, static_keyword)) in
        cases.into_iter().enumerate()
    {
        let mut engine = aura_engine(298_001 + offset as u64);
        let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let aura = cast_and_resolve_aura(&mut engine, card_id, creature, mana);

        assert_eq!(
            engine.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(creature))
        );
        assert_eq!(engine.effective_power(creature), Some(power));
        assert_eq!(engine.effective_toughness(creature), Some(toughness));
        assert!(engine.effective_has_keyword(creature, temporary));
        if let Some(keyword) = static_keyword {
            assert!(engine.effective_has_keyword(creature, keyword));
        }

        end_active_turn(&mut engine, 0);
        assert!(
            !engine.effective_has_keyword(creature, temporary),
            "{card_id} ETB keyword expires at cleanup"
        );
        assert_eq!(engine.effective_power(creature), Some(power));
        assert_eq!(engine.effective_toughness(creature), Some(toughness));
        if let Some(keyword) = static_keyword {
            assert!(
                engine.effective_has_keyword(creature, keyword),
                "{card_id} static attached keyword survives temporary cleanup"
            );
        }
    }
}

#[test]
fn issue_298_aquitects_defenses_is_controller_only_in_three_player_target_publication() {
    let mut engine = three_player_main1(298_010);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_one = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let opponent_two = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "aquitects_defenses");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "aquitects_defenses");
    let legal = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)]
        .groups[0]
        .valid_permanent_ids;
    assert_eq!(legal, &[own]);
    for opponent in [opponent_one, opponent_two] {
        assert!(
            engine
                .apply_command(0, &cast_spell(slot, target_object(opponent)))
                .is_err(),
            "Aquitect's Defenses cannot target an opponent's creature"
        );
    }

    engine
        .apply_command(0, &cast_spell(slot, target_object(own)))
        .expect("Aquitect's Defenses can target a creature controlled by its caster");
    resolve_entire_stack_three_player(&mut engine);
    let aura = battlefield_object_for_card(&engine, 0, "aquitects_defenses");
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(own))
    );
    assert!(engine.effective_has_keyword(own, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(opponent_one, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(opponent_two, Keyword::Hexproof));
}

#[test]
fn issue_298_etb_grant_reads_current_attachment_and_rejects_stale_cast_target() {
    let mut reassigned = aura_engine(298_020);
    let first = inject_creature_on_battlefield(&mut reassigned, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut reassigned, 0, "grizzly_bears");
    let aura = cast_aura_until_etb_trigger(
        &mut reassigned,
        "super_speed",
        first,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        reassigned.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(first))
    );
    reassigned.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(second));
    resolve_entire_stack_two_player(&mut reassigned);
    assert!(!reassigned.effective_has_keyword(first, Keyword::FirstStrike));
    assert!(reassigned.effective_has_keyword(second, Keyword::FirstStrike));
    assert_eq!(reassigned.effective_power(first), Some(2));
    assert_eq!(reassigned.effective_power(second), Some(3));

    let mut stale = aura_engine(298_021);
    let target = inject_creature_on_battlefield(&mut stale, 0, "grizzly_bears");
    let aura_card = inject_card_into_hand(&mut stale, 0, "fae_flight");
    give_mana(
        &mut stale,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&stale, 0, "fae_flight");
    stale
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Fae Flight at the initially legal target");
    move_battlefield_object_to_graveyard(&mut stale, 0, target);
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&aura_card].zone, Zone::Graveyard);
    assert_eq!(stale.state.objects[&aura_card].attached_to, None);
    assert!(!stale.state.players[0].battlefield.contains(&aura_card));
    assert!(stale.state.continuous_effects.is_empty());
}

#[test]
fn issue_298_aquitects_target_becomes_illegal_if_control_changes_before_resolution() {
    let mut engine = aura_engine(298_025);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "aquitects_defenses");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "aquitects_defenses");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Aquitect's Defenses at the initially legal target");

    // Change control while the Aura spell is still on the stack. The target remains the same
    // physical object, but it no longer satisfies the printed "you control" restriction at
    // resolution (CR 608.2b).
    engine.state.players[0]
        .battlefield
        .retain(|candidate| *candidate != target);
    engine.state.players[1].battlefield.push(target);
    let target_object = engine.state.objects.get_mut(&target).expect("target");
    target_object.base_controller = 1;
    target_object.controller = 1;

    resolve_entire_stack_two_player(&mut engine);

    let aura = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "aquitects_defenses")
        .expect("Aquitect's Defenses object");
    assert_eq!(aura.zone, Zone::Graveyard);
    assert_eq!(aura.attached_to, None);
    assert!(engine.state.players[1].battlefield.contains(&target));
    assert!(!engine.effective_has_keyword(target, Keyword::Hexproof));
    assert_eq!(engine.effective_power(target), Some(2));
    assert_eq!(engine.effective_toughness(target), Some(2));
}

#[test]
fn issue_298_aura_leave_and_reenter_keeps_pending_etb_source_generations_distinct() {
    let mut engine = aura_engine(298_026);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let aura = cast_aura_until_etb_trigger(
        &mut engine,
        "super_speed",
        first,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let entered_generation = engine.state.zone_change_generation[&aura];
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(first))
    );

    // Use the real dev relocation funnel so the pending trigger records LKI for the old source
    // generation, drains the old Aura's static effect, and increments the object's generation.
    dev_move_card(&mut engine, 0, "Super Speed", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&aura].attached_to, None);
    assert_eq!(
        engine.state.zone_change_generation[&aura],
        entered_generation + 1
    );

    // Move the same physical card to hand and cast it again. Casting is the real Aura entry path,
    // so the re-entered object is attached before state-based cleanup while its fresh ETB trigger
    // is associated with a new source generation.
    dev_move_card(&mut engine, 0, "Super Speed", DevZone::Hand);
    let hand_generation = engine.state.zone_change_generation[&aura];
    assert_eq!(hand_generation, entered_generation + 2);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "super_speed");
    engine
        .apply_command(0, &cast_spell(slot, target_object(second)))
        .expect("cast the re-entered Super Speed at the second creature");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    let reentered_generation = engine.state.zone_change_generation[&aura];
    assert!(reentered_generation > hand_generation);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(second))
    );
    assert_eq!(
        engine.state.stack.len(),
        2,
        "old and new ETB triggers are pending"
    );

    resolve_entire_stack_two_player(&mut engine);

    assert!(
        engine.effective_has_keyword(first, Keyword::FirstStrike),
        "the old trigger uses the old Aura generation's attached-object LKI"
    );
    assert!(
        engine.effective_has_keyword(second, Keyword::FirstStrike),
        "the re-entered Aura trigger uses its new attachment"
    );
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&aura],
        reentered_generation
    );
}

#[test]
fn issue_298_leave_and_reenter_does_not_reuse_the_old_temporary_keyword_generation() {
    let mut engine = aura_engine(298_030);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let aura = cast_and_resolve_aura(
        &mut engine,
        "super_speed",
        creature,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    assert!(engine.effective_has_keyword(creature, Keyword::FirstStrike));
    let before = engine
        .state
        .zone_change_generation
        .get(&creature)
        .copied()
        .unwrap_or(0);

    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt_slot, target_object(creature)))
        .expect("cast Lightning Bolt at the attached creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert!(engine.state.continuous_effects.is_empty());

    return_object_to_battlefield(&mut engine, 0, creature);
    assert_eq!(engine.state.zone_change_generation[&creature], before + 2);
    assert!(!engine.effective_has_keyword(creature, Keyword::FirstStrike));
    assert_eq!(engine.effective_power(creature), Some(2));
}
