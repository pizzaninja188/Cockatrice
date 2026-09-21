//! Reviewed combat-trick scenarios: Mortify, Withering Torment, Moment of Triumph, Adamant Will,
//! Blossoming Defense, Snakeskin Veil, Take Up the Shield, Maximum Overdrive, Lightfoot Technique
//! and Overprotect.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21. Every expectation is the
//! reviewed printed Oracle behavior. Governance: CR 115.1/608.2b (targeting and revalidation),
//! 701.7 (destroy), 119.3 (life loss/gain), 611.2c and 613 layer 6 (until-end-of-turn keyword
//! grants), and 122.1 (+1/+1 counters).

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

fn trick_engine(seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("forest", &["grizzly_bears", "swiftfoot_boots"]),
    ]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    (e, own, opposing)
}

fn cast_resolve(e: &mut GameEngine, card: &str, target: u32) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, target_object(target)));
    semantic::complete(e, 8, |_| None).require_exercised();
}

#[test]
fn issue_combat_tricks_mortify() {
    let (mut e, _, opposing) = trick_engine(700_001);
    // A creature and an enchantment are both legal; a land is not.
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "crusade");
    cast_resolve(&mut e, "mortify", opposing);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);
    cast_resolve(&mut e, "mortify", enchantment);
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard);

    let (mut land, _, _) = trick_engine(700_002);
    let forest = inject_permanent_on_battlefield(&mut land, 1, "forest");
    inject_card_into_hand(&mut land, 0, "mortify");
    let slot = hand_index_for_card(&land, 0, "mortify");
    assert!(
        land.apply_command(0, &cast_spell(slot, target_object(forest)))
            .is_err(),
        "a land is not a creature or enchantment"
    );
}

#[test]
fn issue_combat_tricks_withering_torment() {
    let (mut e, _, opposing) = trick_engine(700_003);
    let life_before = e.state.players[0].life;
    let opponent_life = e.state.players[1].life;
    cast_resolve(&mut e, "withering_torment", opposing);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, life_before - 2);
    assert_eq!(e.state.players[1].life, opponent_life);

    // An enchantment is the other legal target.
    let (mut enchant, _, _) = trick_engine(700_014);
    let enchantment = inject_permanent_on_battlefield(&mut enchant, 1, "crusade");
    cast_resolve(&mut enchant, "withering_torment", enchantment);
    assert_eq!(enchant.state.objects[&enchantment].zone, Zone::Graveyard);

    // If the target is illegal at resolution the spell doesn't resolve and no life is lost.
    let (mut fizzle, _, target) = trick_engine(700_015);
    inject_card_into_hand(&mut fizzle, 0, "withering_torment");
    let slot = hand_index_for_card(&fizzle, 0, "withering_torment");
    let spell = fizzle.state.players[0].hand[slot];
    let life_before = fizzle.state.players[0].life;
    semantic::accepted(&mut fizzle, 0, &cast_spell(slot, target_object(target)));
    fizzle.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    fizzle.state.players[1].hand.push(target);
    fizzle.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *fizzle
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(fizzle.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(fizzle.state.players[0].life, life_before);
}

#[test]
fn issue_combat_tricks_moment_of_triumph() {
    let (mut e, own, _) = trick_engine(700_004);
    let life_before = e.state.players[0].life;
    cast_resolve(&mut e, "moment_of_triumph", own);
    assert_eq!(e.effective_power(own), Some(4));
    assert_eq!(e.effective_toughness(own), Some(4));
    assert_eq!(e.state.players[0].life, life_before + 2);

    // If the target is illegal at resolution the spell doesn't resolve and no life is gained.
    let (mut fizzle, _, target) = trick_engine(700_016);
    inject_card_into_hand(&mut fizzle, 0, "moment_of_triumph");
    let slot = hand_index_for_card(&fizzle, 0, "moment_of_triumph");
    let spell = fizzle.state.players[0].hand[slot];
    let life_before = fizzle.state.players[0].life;
    semantic::accepted(&mut fizzle, 0, &cast_spell(slot, target_object(target)));
    fizzle.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    fizzle.state.players[1].hand.push(target);
    fizzle.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *fizzle
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(fizzle.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(fizzle.state.players[0].life, life_before);
}

#[test]
fn issue_combat_tricks_adamant_will() {
    let (mut e, own, _) = trick_engine(700_005);
    cast_resolve(&mut e, "adamant_will", own);
    assert_eq!(e.effective_power(own), Some(4));
    assert!(e.effective_has_keyword(own, Keyword::Indestructible));

    // The pump and keyword last only until end of turn.
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(own), Some(2));
    assert_eq!(e.effective_toughness(own), Some(2));
    assert!(!e.effective_has_keyword(own, Keyword::Indestructible));
}

#[test]
fn issue_combat_tricks_blossoming_defense() {
    let (mut e, own, _) = trick_engine(700_006);
    cast_resolve(&mut e, "blossoming_defense", own);
    assert_eq!(e.effective_power(own), Some(4));
    assert!(e.effective_has_keyword(own, Keyword::Hexproof));

    let (mut illegal, _, opposing2) = trick_engine(700_007);
    inject_card_into_hand(&mut illegal, 0, "blossoming_defense");
    let slot = hand_index_for_card(&illegal, 0, "blossoming_defense");
    assert!(
        illegal
            .apply_command(0, &cast_spell(slot, target_object(opposing2)))
            .is_err(),
        "an opponent's creature is not a legal target"
    );
}

#[test]
fn issue_combat_tricks_snakeskin_veil() {
    let (mut e, own, _) = trick_engine(700_008);
    cast_resolve(&mut e, "snakeskin_veil", own);
    assert_eq!(
        e.state.objects[&own]
            .counters
            .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert_eq!(e.effective_power(own), Some(3));
    assert!(e.effective_has_keyword(own, Keyword::Hexproof));

    let (mut illegal, _, opposing) = trick_engine(700_009);
    inject_card_into_hand(&mut illegal, 0, "snakeskin_veil");
    let slot = hand_index_for_card(&illegal, 0, "snakeskin_veil");
    assert!(
        illegal
            .apply_command(0, &cast_spell(slot, target_object(opposing)))
            .is_err(),
        "an opponent's creature is not a legal target"
    );
}

#[test]
fn issue_combat_tricks_take_up_the_shield() {
    let (mut e, own, _) = trick_engine(700_010);
    cast_resolve(&mut e, "take_up_the_shield", own);
    assert_eq!(
        e.state.objects[&own]
            .counters
            .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert!(e.effective_has_keyword(own, Keyword::Lifelink));
    assert!(e.effective_has_keyword(own, Keyword::Indestructible));
}

#[test]
fn issue_combat_tricks_maximum_overdrive() {
    let (mut e, own, _) = trick_engine(700_011);
    cast_resolve(&mut e, "maximum_overdrive", own);
    assert_eq!(
        e.state.objects[&own]
            .counters
            .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert!(e.effective_has_keyword(own, Keyword::Deathtouch));
    assert!(e.effective_has_keyword(own, Keyword::Indestructible));
}

#[test]
fn issue_combat_tricks_lightfoot_technique() {
    let (mut e, own, _) = trick_engine(700_012);
    cast_resolve(&mut e, "lightfoot_technique", own);
    assert_eq!(
        e.state.objects[&own]
            .counters
            .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert!(e.effective_has_keyword(own, Keyword::Flying));
    assert!(e.effective_has_keyword(own, Keyword::Indestructible));
}

#[test]
fn issue_combat_tricks_overprotect() {
    let (mut e, own, _) = trick_engine(700_013);
    cast_resolve(&mut e, "overprotect", own);
    assert_eq!(e.effective_power(own), Some(5));
    assert_eq!(e.effective_toughness(own), Some(5));
    assert!(e.effective_has_keyword(own, Keyword::Trample));
    assert!(e.effective_has_keyword(own, Keyword::Hexproof));
    assert!(e.effective_has_keyword(own, Keyword::Indestructible));

    let (mut illegal, _, opposing) = trick_engine(700_017);
    inject_card_into_hand(&mut illegal, 0, "overprotect");
    let slot = hand_index_for_card(&illegal, 0, "overprotect");
    assert!(
        illegal
            .apply_command(0, &cast_spell(slot, target_object(opposing)))
            .is_err(),
        "an opponent's creature is not a legal target"
    );
}
