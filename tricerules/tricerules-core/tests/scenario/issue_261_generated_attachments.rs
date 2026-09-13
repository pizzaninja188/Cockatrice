//! Issue #261 — generated Auras and Equipment reuse the engine's attachment, target,
//! continuous-modifier, and intrinsic Equip timing contracts.
//!
//! Oracle and rulings were checked 2026-09-12. CR 115.1b, 301.5, 303.4, 601.2c,
//! 608.2b, 613.1f, 613.4c, 702.5, 702.6, and 704.5m-n govern Aura targeting and
//! revalidation, attachment legality, continuous modifiers, Equip, and state-based cleanup.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

fn attachment_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn move_battlefield_object_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("object")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_insert(0) += 1;
}

fn advance_to_upkeep(engine: &mut GameEngine, player: i32) {
    for _ in 0..50 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = engine
                    .state
                    .player_idx(cleanup_player)
                    .expect("cleanup player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine.apply_command(actor, &command).expect("advance turn");
        if engine.state.active_player_id() == player
            && engine.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

#[test]
fn generated_charmed_sleep_publishes_revalidates_and_cleans_up_its_attachment() {
    let mut stale = attachment_engine(261_001);
    let legal_creature = inject_creature_on_battlefield(&mut stale, 0, "grizzly_bears");
    let stale_creature = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut stale, 0, "short_sword");
    inject_card_into_hand(&mut stale, 0, "charmed_sleep");
    give_mana(
        &mut stale,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&stale, 0, "charmed_sleep");
    let published = &stale.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)]
        .groups[0]
        .valid_permanent_ids;
    assert!(published.contains(&legal_creature));
    assert!(published.contains(&stale_creature));
    assert!(!published.contains(&artifact));

    stale
        .apply_command(0, &cast_spell(slot, target_object(stale_creature)))
        .expect("cast Charmed Sleep");
    move_battlefield_object_to_graveyard(&mut stale, 1, stale_creature);
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(count_card_id_in_graveyard(&stale, 0, "charmed_sleep"), 1);
    assert_eq!(
        stale.state.players[0]
            .battlefield
            .iter()
            .filter(|object_id| stale.state.objects[object_id].card_id == "charmed_sleep")
            .count(),
        0,
        "an Aura whose only target became stale must not enter"
    );

    let mut attached = attachment_engine(261_002);
    let creature = inject_creature_on_battlefield(&mut attached, 0, "grizzly_bears");
    inject_card_into_hand(&mut attached, 0, "charmed_sleep");
    give_mana(
        &mut attached,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&attached, 0, "charmed_sleep");
    attached
        .apply_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Charmed Sleep");
    resolve_entire_stack_two_player(&mut attached);
    let aura = battlefield_object_for_card(&attached, 0, "charmed_sleep");
    assert_eq!(
        attached.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
    assert!(
        attached.state.objects[&creature].tapped,
        "the Aura's ETB trigger taps its attached creature"
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut attached, 0, creature),
        ["Doesn't untap during its controller's untap step"]
    );

    inject_card_into_hand(&mut attached, 0, "vitalize");
    cast_instant_and_resolve(
        &mut attached,
        0,
        "vitalize",
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    assert!(
        !attached.state.objects[&creature].tapped,
        "the untap-step restriction is not a universal untap prohibition"
    );
    attached
        .state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .tapped = true;
    advance_to_upkeep(&mut attached, 0);
    assert!(
        attached.state.objects[&creature].tapped,
        "the creature stays tapped during its controller's untap step"
    );

    move_battlefield_object_to_graveyard(&mut attached, 0, creature);
    attached
        .apply_command(attached.state.priority_player_id(), &pass())
        .expect("state-based actions clean up the Aura");
    assert_eq!(attached.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(attached.state.objects[&aura].attached_to, None);
}

#[test]
fn generated_short_bow_enforces_atomic_sorcery_speed_equip_and_moves_its_modifier() {
    let mut engine = attachment_engine(261_003);
    inject_card_into_hand(&mut engine, 0, "short_bow");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "short_bow");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Short Bow");
    resolve_entire_stack_two_player(&mut engine);
    let bow = battlefield_object_for_card(&engine, 0, "short_bow");
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    let ability_key = u64::from(bow) << 32;
    let published = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&ability_key]
        .groups[0]
        .valid_permanent_ids;
    assert!(published.contains(&first));
    assert!(published.contains(&second));
    assert!(!published.contains(&opponent));

    assert!(
        apply_ability(&mut engine, 0, bow, 0, target_object(first)).is_err(),
        "unpayable Equip must fail before changing attachment state"
    );
    assert_eq!(engine.state.objects[&bow].attached_to, None);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, bow, 0, target_object(first)).expect("equip first creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(first), Some(3));
    assert_eq!(engine.effective_toughness(first), Some(3));
    assert!(engine.effective_has_keyword(first, Keyword::Reach));
    assert!(engine.effective_has_keyword(first, Keyword::Vigilance));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, bow, 0, target_object(second)).expect("re-equip");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&bow].attached_to,
        Some(AttachmentRecipient::Object(second))
    );
    assert_eq!(engine.effective_power(first), Some(2));
    assert!(!engine.effective_has_keyword(first, Keyword::Reach));
    assert_eq!(engine.effective_power(second), Some(3));
    assert!(engine.effective_has_keyword(second, Keyword::Reach));

    end_active_turn(&mut engine, 0);
    assert!(!zone_view_ability_flags(&mut engine, 0, bow)[0]);
    assert!(
        apply_ability(&mut engine, 0, bow, 0, target_object(first)).is_err(),
        "Equip is unavailable outside its controller's sorcery-speed window"
    );
    assert_eq!(
        engine.state.objects[&bow].attached_to,
        Some(AttachmentRecipient::Object(second))
    );
}
