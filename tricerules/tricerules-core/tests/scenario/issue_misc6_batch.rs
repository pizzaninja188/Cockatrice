//! Reviewed direct-RON activated-ability scenarios: Patriot, Shield Wielder, Brave-Kin Duo,
//! Raccoon Rallier, Vampiric Rites, Deserter's Disciple, Stark Industries Executive,
//! Treasure Dredger and Flame-Chain Mauler.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! CR 118.12/119 (life payments), CR 301.5 (Treasure), CR 602 (activated abilities),
//! CR 608.2b (target revalidation), and CR 611.2c (until end of turn).

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn add_plus_one_counter(e: &mut GameEngine, oid: u32) {
    e.state
        .objects
        .get_mut(&oid)
        .expect("object")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
}

#[test]
fn issue_misc6_patriot_pumps_and_grants_hexproof_to_another_creature() {
    let mut e = engine(730_001);
    let patriot = inject_creature_on_battlefield(&mut e, 0, "patriot,_shield_wielder");
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    semantic::accepted(
        &mut e,
        0,
        &activate_ability(patriot, 0, target_object(target)),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(4), "+2 power");
    assert_eq!(
        e.effective_toughness(target),
        Some(2),
        "toughness unchanged"
    );
    assert!(e.effective_has_keyword(target, Keyword::Hexproof));

    // "Another target creature you control": the source and opponent creatures are illegal.
    let mut bad = engine(730_011);
    let patriot = inject_creature_on_battlefield(&mut bad, 0, "patriot,_shield_wielder");
    let opponent = inject_creature_on_battlefield(&mut bad, 1, "grizzly_bears");
    grant_pool(&mut bad, 0);
    assert!(
        bad.apply_command(0, &activate_ability(patriot, 0, target_object(patriot)))
            .is_err(),
        "the source is not 'another' creature"
    );
    assert!(
        bad.apply_command(0, &activate_ability(patriot, 0, target_object(opponent)))
            .is_err(),
        "an opponent creature is outside the You filter"
    );
}

#[test]
fn issue_misc6_brave_kin_duo_pumps_a_target_at_sorcery_speed() {
    let mut e = engine(730_002);
    let duo = inject_creature_on_battlefield(&mut e, 0, "brave-kin_duo");
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    semantic::accepted(&mut e, 0, &activate_ability(duo, 0, target_object(target)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(3), "+1/+1");
    assert_eq!(e.effective_toughness(target), Some(3));
    assert!(e.state.objects[&duo].tapped, "the tap cost was paid");

    // Sorcery speed: it cannot be activated once the combat step has begun.
    let mut combat = GameEngine::new(730_012, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut combat);
    let duo = inject_creature_on_battlefield(&mut combat, 0, "brave-kin_duo");
    let target = inject_creature_on_battlefield(&mut combat, 0, "grizzly_bears");
    assert!(
        combat
            .apply_command(0, &activate_ability(duo, 0, target_object(target)))
            .is_err(),
        "sorcery-speed activation is illegal in the declare-attackers step"
    );
}

#[test]
fn issue_misc6_raccoon_rallier_grants_haste_at_sorcery_speed() {
    let mut e = engine(730_003);
    let rallier = inject_creature_on_battlefield(&mut e, 0, "raccoon_rallier");
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    grant_pool(&mut e, 0);
    semantic::accepted(
        &mut e,
        0,
        &activate_ability(rallier, 0, target_object(target)),
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(target, Keyword::Haste));
    assert!(!e.effective_has_keyword(opponent, Keyword::Haste));

    let mut bad = engine(730_013);
    let rallier = inject_creature_on_battlefield(&mut bad, 0, "raccoon_rallier");
    let opponent = inject_creature_on_battlefield(&mut bad, 1, "grizzly_bears");
    grant_pool(&mut bad, 0);
    assert!(
        bad.apply_command(0, &activate_ability(rallier, 0, target_object(opponent)))
            .is_err(),
        "an opponent creature is outside the You filter"
    );

    // Sorcery speed: it cannot be activated once the combat step has begun.
    let mut combat = GameEngine::new(730_023, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut combat);
    let rallier = inject_creature_on_battlefield(&mut combat, 0, "raccoon_rallier");
    let target = inject_creature_on_battlefield(&mut combat, 0, "grizzly_bears");
    assert!(
        combat
            .apply_command(0, &activate_ability(rallier, 0, target_object(target)))
            .is_err(),
        "sorcery-speed activation is illegal in the declare-attackers step"
    );
}

#[test]
fn issue_misc6_vampiric_rites_sacrifices_for_life_and_a_card() {
    let mut e = engine(730_004);
    let rites = inject_permanent_on_battlefield(&mut e, 0, "vampiric_rites");
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let life_before = e.state.players[0].life;
    let hand_before = e.state.players[0].hand.len();
    semantic::accepted(
        &mut e,
        0,
        &activate_ability_with_costs(rites, 0, vec![], vec![permanent_cost_selection(1, fodder)]),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&fodder].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, life_before + 1);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);

    // With no creature, the sacrifice cost cannot be paid.
    let mut alone = engine(730_014);
    let rites = inject_permanent_on_battlefield(&mut alone, 0, "vampiric_rites");
    grant_pool(&mut alone, 0);
    assert!(
        alone
            .apply_command(0, &activate_ability(rites, 0, vec![]))
            .is_err(),
        "no creature is available to sacrifice"
    );
}

#[test]
fn issue_misc6_deserters_disciple_makes_a_small_attacker_unblockable() {
    let mut e = engine(730_005);
    let disciple = inject_creature_on_battlefield(&mut e, 0, "deserters_disciple");
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    semantic::accepted(
        &mut e,
        0,
        &activate_ability(disciple, 0, target_object(target)),
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(
        zone_view_rules_annotation_labels(&mut e, 0, target)
            .iter()
            .any(|label| label == "Can't be blocked"),
        "the resolved restriction is published"
    );

    // The source is excluded ("another") and a power-3 creature is outside the bound.
    let mut bad = engine(730_015);
    let disciple = inject_creature_on_battlefield(&mut bad, 0, "deserters_disciple");
    let big = inject_creature_on_battlefield(&mut bad, 0, "grizzly_bears");
    add_plus_one_counter(&mut bad, big);
    grant_pool(&mut bad, 0);
    assert!(
        bad.apply_command(0, &activate_ability(disciple, 0, target_object(disciple)))
            .is_err(),
        "the source is not 'another' creature"
    );
    assert!(
        bad.apply_command(0, &activate_ability(disciple, 0, target_object(big)))
            .is_err(),
        "power 3 is outside the power-2-or-less bound"
    );

    // CR 608.2b / 2025-10-02 ruling: the bound is revalidated at resolution.
    let mut fizzle = engine(730_025);
    let disciple = inject_creature_on_battlefield(&mut fizzle, 0, "deserters_disciple");
    let target = inject_creature_on_battlefield(&mut fizzle, 0, "grizzly_bears");
    grant_pool(&mut fizzle, 0);
    semantic::accepted(
        &mut fizzle,
        0,
        &activate_ability(disciple, 0, target_object(target)),
    );
    add_plus_one_counter(&mut fizzle, target);
    resolve_entire_stack_two_player(&mut fizzle);
    assert!(
        zone_view_rules_annotation_labels(&mut fizzle, 0, target).is_empty(),
        "a target that grows above power 2 before resolution makes the ability resolve with no effect"
    );
}

#[test]
fn issue_misc6_stark_industries_executive_creates_a_treasure() {
    let mut e = engine(730_006);
    let executive = inject_creature_on_battlefield(&mut e, 0, "stark_industries_executive");
    grant_pool(&mut e, 0);
    semantic::accepted(&mut e, 0, &activate_ability(executive, 0, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
}

#[test]
fn issue_misc6_treasure_dredger_pays_life_for_a_treasure() {
    let mut e = engine(730_007);
    let dredger = inject_creature_on_battlefield(&mut e, 0, "treasure_dredger");
    grant_pool(&mut e, 0);
    let life_before = e.state.players[0].life;
    semantic::accepted(&mut e, 0, &activate_ability(dredger, 0, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, life_before - 1, "pay 1 life");
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert!(e.state.objects[&dredger].tapped);
}

#[test]
fn issue_misc6_flame_chain_mauler_pumps_itself_and_gains_menace() {
    let mut e = engine(730_008);
    let mauler = inject_creature_on_battlefield(&mut e, 0, "flame-chain_mauler");
    grant_pool(&mut e, 0);
    let base_power = e.effective_power(mauler).expect("power");
    let base_toughness = e.effective_toughness(mauler).expect("toughness");
    semantic::accepted(&mut e, 0, &activate_ability(mauler, 0, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(mauler), Some(base_power + 1), "+1 power");
    assert_eq!(
        e.effective_toughness(mauler),
        Some(base_toughness),
        "toughness unchanged"
    );
    assert!(e.effective_has_keyword(mauler, Keyword::Menace));
}
