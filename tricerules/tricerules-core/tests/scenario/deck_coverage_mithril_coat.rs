//! Actual-card coverage for Mithril Coat's flash casting, legendary ETB attachment, and Equip.
//!
//! Wizards' LTR release notes confirm that Coat may be cast without creatures, does not enter
//! attached, its ETB attachment is not Equip, and an illegal target leaves it unattached. CR
//! 301.5b covers Equipment entering unattached; 115.1d/603.3d cover the targeted trigger and its
//! no-legal-target case; 608.2b rechecks the target; 613.1f applies the granted keyword;
//! 702.6a makes Equip sorcery-speed, 702.8a defines Flash timing, and 702.12a-b define indestructible.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{DevCommand, DevMoveCard, DevZone};

const MITHRIL_COAT: &str = "mithril_coat";
const LEGENDARY_CREATURE: &str = "isamaru,_hound_of_konda";
const NONLEGENDARY_CREATURE: &str = "grizzly_bears";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: target_object(object_id),
            ..Default::default()
        })),
    }
}

fn move_isamaru_to_graveyard(engine: &mut GameEngine) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: "Isamaru, Hound of Konda".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the selected creature out of the battlefield");
}

#[test]
fn mithril_coat_casts_with_flash_without_creatures_and_keeps_its_own_indestructible() {
    let mut engine = engine(202_609_330);
    pass_both_players(&mut engine);
    assert_ne!(
        engine.state.turn_step,
        tricerules_core::TurnStep::Main1,
        "cast during a non-main step"
    );

    let coat = inject_card_into_hand(&mut engine, 0, MITHRIL_COAT);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, MITHRIL_COAT);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("Flash lets Mithril Coat be cast at instant timing with no creatures");
    assert_eq!(engine.state.objects[&coat].zone, Zone::Stack);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&coat].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&coat].attached_to, None);
    assert!(engine.effective_has_keyword(coat, Keyword::Indestructible));
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.blocking_choice().is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn mithril_coat_etb_selects_only_a_legendary_creature_and_grants_indestructible() {
    let mut engine = engine(202_609_331);
    let legendary = inject_creature_on_battlefield(&mut engine, 0, LEGENDARY_CREATURE);
    let nonlegendary = inject_creature_on_battlefield(&mut engine, 0, NONLEGENDARY_CREATURE);
    let coat = inject_card_into_hand(&mut engine, 0, MITHRIL_COAT);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, MITHRIL_COAT);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Mithril Coat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&coat].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&coat].attached_to, None);
    assert_eq!(engine.state.pending_triggers.len(), 1);

    let ability_key = u64::from(coat) << 32;
    let legal_targets = &engine.initial_response_batch().legal_by_player[&0]
        .valid_targets_by_ability[&ability_key]
        .groups[0]
        .valid_permanent_ids;
    assert_eq!(legal_targets, &[legendary]);
    assert!(!legal_targets.contains(&nonlegendary));

    engine
        .apply_command(0, &choose_trigger_target(legendary))
        .expect("choose the only legal legendary creature");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&coat].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(legendary))
    );
    assert!(engine.effective_has_keyword(legendary, Keyword::Indestructible));
    assert!(!engine.effective_has_keyword(nonlegendary, Keyword::Indestructible));
    assert_eq!(engine.state.objects[&nonlegendary].zone, Zone::Battlefield);
}

#[test]
fn mithril_coat_remains_unattached_if_its_etb_target_becomes_illegal() {
    let mut engine = engine(202_609_332);
    let legendary = inject_creature_on_battlefield(&mut engine, 0, LEGENDARY_CREATURE);
    let coat = inject_card_into_hand(&mut engine, 0, MITHRIL_COAT);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, MITHRIL_COAT);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Mithril Coat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_target(legendary))
        .expect("choose the legal legendary target");
    assert_eq!(engine.state.stack.len(), 1);

    move_isamaru_to_graveyard(&mut engine);
    assert_eq!(engine.state.objects[&legendary].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&coat].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&coat].attached_to, None);
    assert!(engine.effective_has_keyword(coat, Keyword::Indestructible));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn mithril_coat_equip_three_targets_any_controlled_creature_at_sorcery_speed() {
    let mut engine = engine(202_609_333);
    let nonlegendary = inject_creature_on_battlefield(&mut engine, 0, NONLEGENDARY_CREATURE);
    let coat = inject_card_into_hand(&mut engine, 0, MITHRIL_COAT);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, MITHRIL_COAT);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Mithril Coat");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&coat].attached_to, None);
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);

    apply_ability(&mut engine, 0, coat, 0, target_object(nonlegendary))
        .expect("Equip may target a nonlegendary creature you control");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&coat].attached_to, None);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&coat].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(nonlegendary))
    );
    assert!(engine.effective_has_keyword(nonlegendary, Keyword::Indestructible));

    pass_both_players(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, coat, 0, target_object(nonlegendary))
        .expect_err("Equip is sorcery-speed even though the target is legal");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert_eq!(
        engine.state.objects[&coat].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(nonlegendary))
    );
    assert!(engine.state.stack.is_empty());
}
