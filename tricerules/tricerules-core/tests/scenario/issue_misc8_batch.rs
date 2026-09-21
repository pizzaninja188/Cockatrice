//! Reviewed direct-RON targeted-spell scenarios: Hour of Defeat, Unsubtle Mockery, Reckless
//! Ransacking, Savor, Masterful Flourish, Kin-Tree Severance, Joust Through and Spider Food.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! CR 118/119 (life), CR 120 (damage), CR 202.3 (mana value), CR 301.5 (Treasure/Food),
//! CR 508/509 (combat designations), CR 608.2b (target revalidation), CR 611.2c (until end of
//! turn), CR 701.25 (surveil), and CR 702.12b (indestructible).

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// Casts a targeted instant and resolves it, returning the batch that may hold a surveil choice.
fn cast_and_resolve(
    e: &mut GameEngine,
    caster: i32,
    card_id: &str,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) -> RuledEventBatch {
    inject_card_into_hand(e, caster as usize, card_id);
    grant_pool(e, caster as usize);
    let slot = hand_index_for_card(e, caster as usize, card_id);
    semantic::accepted(e, caster, &cast_spell(slot, targets));
    e.apply_command(caster, &pass()).expect("caster pass");
    e.apply_command(1 - caster, &pass()).expect("other pass")
}

#[test]
fn issue_misc8_hour_of_defeat_destroys_then_surveils() {
    let mut e = engine(732_001);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let batch = cast_and_resolve(&mut e, 0, "hour_of_defeat", target_object(bear));
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));
    assert!(e.state.blocking_choice().is_none(), "the spell completed");
}

#[test]
fn issue_misc8_unsubtle_mockery_deals_four_then_surveils() {
    let mut e = engine(732_002);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let batch = cast_and_resolve(&mut e, 0, "unsubtle_mockery", target_object(bear));
    let choice = find_resolution_choice(&batch).expect("surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));
    assert!(e.state.blocking_choice().is_none());
    assert_eq!(
        e.state.objects[&bear].zone,
        Zone::Graveyard,
        "4 damage destroys a 2/2 once state-based actions run"
    );
}

#[test]
fn issue_misc8_reckless_ransacking_pumps_and_makes_a_treasure() {
    let mut e = engine(732_003);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let base_power = e.effective_power(bear).expect("power");
    let base_toughness = e.effective_toughness(bear).expect("toughness");
    cast_and_resolve(&mut e, 0, "reckless_ransacking", target_object(bear));
    assert_eq!(e.effective_power(bear), Some(base_power + 3));
    assert_eq!(e.effective_toughness(bear), Some(base_toughness + 2));
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
}

#[test]
fn issue_misc8_savor_shrinks_and_makes_food() {
    let mut e = engine(732_004);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(&mut e, 0, "savor", target_object(bear));
    assert_eq!(
        e.state.objects[&bear].zone,
        Zone::Graveyard,
        "-2/-2 reduces a 2/2 to 0 toughness"
    );
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
}

#[test]
fn issue_misc8_masterful_flourish_pumps_and_grants_indestructible() {
    let mut e = engine(732_005);
    let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let base_power = e.effective_power(mine).expect("power");
    cast_and_resolve(&mut e, 0, "masterful_flourish", target_object(mine));
    assert_eq!(e.effective_power(mine), Some(base_power + 1));
    assert!(e.effective_has_keyword(mine, Keyword::Indestructible));

    // An opponent's creature is outside the You filter.
    let mut bad = engine(732_015);
    let theirs = inject_creature_on_battlefield(&mut bad, 1, "grizzly_bears");
    inject_card_into_hand(&mut bad, 0, "masterful_flourish");
    grant_pool(&mut bad, 0);
    let slot = hand_index_for_card(&bad, 0, "masterful_flourish");
    assert!(
        bad.apply_command(0, &cast_spell(slot, target_object(theirs)))
            .is_err(),
        "an opponent creature is not a legal target"
    );
    assert_eq!(bad.effective_power(theirs), Some(2));
}

#[test]
fn issue_misc8_kin_tree_severance_exiles_a_large_permanent() {
    let mut e = engine(732_006);
    let big = inject_permanent_on_battlefield(&mut e, 1, "fateful_discovery");
    cast_and_resolve(&mut e, 0, "kin-tree_severance", target_object(big));
    assert_eq!(e.state.objects[&big].zone, Zone::Exile);

    // A permanent with mana value below 3 is not a legal target.
    let mut bad = engine(732_016);
    let small = inject_creature_on_battlefield(&mut bad, 1, "grizzly_bears");
    inject_card_into_hand(&mut bad, 0, "kin-tree_severance");
    grant_pool(&mut bad, 0);
    let slot = hand_index_for_card(&bad, 0, "kin-tree_severance");
    assert!(
        bad.apply_command(0, &cast_spell(slot, target_object(small)))
            .is_err(),
        "mana value 2 is below the bound"
    );
}

#[test]
fn issue_misc8_joust_through_hits_an_attacker_and_gains_life() {
    let mut e = GameEngine::new(732_007, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    let life_before = e.state.players[0].life;
    inject_card_into_hand(&mut e, 0, "joust_through");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "joust_through");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(attacker)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&attacker].zone,
        Zone::Graveyard,
        "3 damage destroys a 2/2 attacker"
    );
    assert_eq!(e.state.players[0].life, life_before + 1);

    // A nonattacking creature is not a legal target.
    let mut bad = engine(732_017);
    let bench = inject_creature_on_battlefield(&mut bad, 1, "grizzly_bears");
    inject_card_into_hand(&mut bad, 0, "joust_through");
    grant_pool(&mut bad, 0);
    let slot = hand_index_for_card(&bad, 0, "joust_through");
    assert!(
        bad.apply_command(0, &cast_spell(slot, target_object(bench)))
            .is_err(),
        "a nonattacking creature is not attacking or blocking"
    );
}

#[test]
fn issue_misc8_spider_food_destroys_or_makes_food() {
    // With a target artifact: destroy it and create a Food.
    let mut e = engine(732_008);
    let boots = inject_permanent_on_battlefield(&mut e, 1, "swiftfoot_boots");
    cast_and_resolve(&mut e, 0, "spider_food", target_object(boots));
    assert_eq!(e.state.objects[&boots].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);

    // With no target: the spell still resolves and creates a Food.
    let mut no_target = engine(732_018);
    cast_and_resolve(&mut no_target, 0, "spider_food", vec![]);
    assert_eq!(battlefield_token_oids(&no_target, 0, "food").len(), 1);

    // A creature without flying is not a legal target.
    let mut bad = engine(732_028);
    let bear = inject_creature_on_battlefield(&mut bad, 1, "grizzly_bears");
    inject_card_into_hand(&mut bad, 0, "spider_food");
    grant_pool(&mut bad, 0);
    let slot = hand_index_for_card(&bad, 0, "spider_food");
    assert!(
        bad.apply_command(0, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "a ground creature is not an artifact, enchantment, or flier"
    );
}
