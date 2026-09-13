//! Issue #262 — generated targeted combat tricks reuse the shared target schema and ordered
//! temporary pump, keyword, untap, and draw effects.
//!
//! Oracle and rulings were checked 2026-09-12. CR 115, 121, 601.2c, 608.2b-c, 611.2c,
//! 613.1f, 613.4c, 701.21, 702.2, 702.12, 702.18, and 702.19 govern targeting and
//! revalidation, printed resolution order, drawing, temporary P/T and keyword effects, untap,
//! deathtouch, indestructible, reach, trample, and cleanup expiry.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

fn combat_trick_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn fund_spells(engine: &mut GameEngine) {
    give_mana(
        engine,
        0,
        ManaGift {
            w: 4,
            u: 4,
            b: 8,
            g: 4,
            c: 8,
            ..Default::default()
        },
    );
}

fn cast_generated(engine: &mut GameEngine, card_id: &str, target: u32) {
    inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
}

#[test]
fn magic_damper_publishes_only_controlled_creatures_then_untaps_and_expires() {
    let mut engine = combat_trick_engine(262_001);
    fund_spells(&mut engine);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "short_sword");
    engine
        .state
        .objects
        .get_mut(&own)
        .expect("own creature")
        .tapped = true;
    inject_card_into_hand(&mut engine, 0, "magic_damper");
    let slot = hand_index_for_card(&engine, 0, "magic_damper");

    let legal = &engine.initial_response_batch().legal_by_player[&0];
    assert_eq!(
        legal.valid_targets_by_hand_slot[&((slot as u32) << 8)].groups[0].valid_permanent_ids,
        [own]
    );
    assert!(engine
        .apply_command(0, &cast_spell(slot, target_object(opponent)))
        .is_err());
    assert!(engine
        .apply_command(0, &cast_spell(slot, target_object(artifact)))
        .is_err());

    engine
        .apply_command(0, &cast_spell(slot, target_object(own)))
        .expect("controlled creature is legal");
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects[&own].tapped);
    assert_eq!(engine.effective_power(own), Some(3));
    assert_eq!(engine.effective_toughness(own), Some(3));
    assert!(engine.effective_has_keyword(own, Keyword::Hexproof));

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(own), Some(2));
    assert_eq!(engine.effective_toughness(own), Some(2));
    assert!(!engine.effective_has_keyword(own, Keyword::Hexproof));
}

#[test]
fn rebellious_strike_revalidates_before_pumping_or_drawing() {
    let mut stale = combat_trick_engine(262_002);
    fund_spells(&mut stale);
    let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    cast_generated(&mut stale, "rebellious_strike", target);
    let hand_before = stale.state.players[0].hand.len();
    stale.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    stale.state.players[1].hand.push(target);
    stale.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *stale
        .state
        .zone_change_generation
        .entry(target)
        .or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.players[0].hand.len(), hand_before);
    assert!(stale.state.continuous_effects.is_empty());

    let mut success = combat_trick_engine(262_003);
    fund_spells(&mut success);
    let target = inject_creature_on_battlefield(&mut success, 1, "grizzly_bears");
    let drawn = inject_library_card(&mut success, 0, "forest");
    success.state.players[0]
        .library
        .retain(|object_id| *object_id != drawn);
    success.state.players[0].library.push_front(drawn);
    cast_generated(&mut success, "rebellious_strike", target);
    let hand_before = success.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut success);
    assert_eq!(success.effective_power(target), Some(5));
    assert_eq!(success.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(success.state.objects[&drawn].zone, Zone::Hand);
}

#[test]
fn offer_immortality_grants_both_keywords_and_their_distinct_rules_behavior() {
    let mut engine = combat_trick_engine(262_004);
    fund_spells(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "prodigal_sorcerer");
    let victim = inject_creature_with_stats(&mut engine, 1, "hill_giant", 3, 3);

    cast_generated(&mut engine, "offer_immortality", source);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(source, Keyword::Deathtouch));
    assert!(engine.effective_has_keyword(source, Keyword::Indestructible));

    cast_generated(&mut engine, "murder", source);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);

    engine
        .apply_command(0, &activate_ability(source, 0, target_object(victim)))
        .expect("activate deathtouch pinger");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&victim].zone, Zone::Graveyard);

    cast_generated(&mut engine, "last_gasp", source);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Graveyard,
        "indestructible does not stop the zero-toughness state-based action"
    );
}
