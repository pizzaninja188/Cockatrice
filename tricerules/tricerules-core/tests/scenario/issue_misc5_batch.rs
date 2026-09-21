//! Reviewed direct-RON scenarios: Acolyte of Aclazotz, Bartolome del Presidio, Captain Storm,
//! Cosmium Raider, Ashiok's Reaper, Battlesong Berserker, Clammy Prowler and Bake into a Pie.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! 118.12 (additional costs), 119/120 (life), 122.1 (counters), 301.5 (Food), 508 (attack
//! triggers), 602 (activated abilities), 603.6 (entry/leaves triggers), 608.2b (target
//! revalidation), 609.3 (intervening if), and 611.2c (until end of turn).

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, TargetRef, TargetRefKind};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

#[test]
fn issue_misc5_acolyte_of_aclazotz_sacrifices_another_permanent() {
    let mut e = engine(729_001);
    let acolyte = inject_creature_on_battlefield(&mut e, 0, "acolyte_of_aclazotz");
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let p0_before = e.state.players[0].life;
    let p1_before = e.state.players[1].life;
    semantic::accepted(
        &mut e,
        0,
        &activate_ability_with_costs(
            acolyte,
            0,
            vec![],
            vec![permanent_cost_selection(1, fodder)],
        ),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        p1_before - 1,
        "each opponent loses one"
    );
    assert_eq!(
        e.state.players[0].life,
        p0_before + 1,
        "the controller gains one"
    );
    assert_eq!(e.state.objects[&fodder].zone, Zone::Graveyard);
    assert!(e.state.objects[&acolyte].tapped, "the tap cost was paid");

    // The source alone cannot pay the "another creature or artifact" cost.
    let mut alone = engine(729_011);
    let acolyte = inject_creature_on_battlefield(&mut alone, 0, "acolyte_of_aclazotz");
    assert!(
        alone
            .apply_command(0, &activate_ability(acolyte, 0, vec![]))
            .is_err(),
        "the source is excluded from its own sacrifice cost"
    );
}

#[test]
fn issue_misc5_bartolome_del_presidio_grows_after_sacrificing() {
    let mut e = engine(729_002);
    let bartolome = inject_creature_on_battlefield(&mut e, 0, "bartolome_del_presidio");
    let artifact = inject_permanent_on_battlefield(&mut e, 0, "swiftfoot_boots");
    semantic::accepted(
        &mut e,
        0,
        &activate_ability_with_costs(
            bartolome,
            0,
            vec![],
            vec![permanent_cost_selection(0, artifact)],
        ),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&bartolome].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);

    // No other creature or artifact: the sacrifice cannot be paid.
    let mut alone = engine(729_012);
    let bartolome = inject_creature_on_battlefield(&mut alone, 0, "bartolome_del_presidio");
    assert!(
        alone
            .apply_command(0, &activate_ability(bartolome, 0, vec![]))
            .is_err(),
        "the source is the only permanent and is excluded"
    );
}

#[test]
fn issue_misc5_captain_storm_triggers_on_a_controlled_artifact() {
    let mut e = engine(729_003);
    let storm = inject_creature_on_battlefield(&mut e, 0, "captain_storm,_cosmium_raider");
    inject_card_into_hand(&mut e, 0, "swiftfoot_boots");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "swiftfoot_boots");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    e.apply_command(0, &pass()).expect("caster passes");
    e.apply_command(1, &pass()).expect("the artifact resolves");
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "one artifact entry triggers"
    );
    semantic::accepted(&mut e, 0, &choose_trigger_target(storm));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&storm].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "a Pirate you control is a legal target"
    );

    // A nonartifact permanent entering does not trigger the watcher.
    let mut no_trigger = engine(729_013);
    inject_creature_on_battlefield(&mut no_trigger, 0, "captain_storm,_cosmium_raider");
    inject_card_into_hand(&mut no_trigger, 0, "grizzly_bears");
    grant_pool(&mut no_trigger, 0);
    let slot = hand_index_for_card(&no_trigger, 0, "grizzly_bears");
    semantic::accepted(&mut no_trigger, 0, &cast_spell(slot, vec![]));
    no_trigger.apply_command(0, &pass()).expect("caster passes");
    no_trigger
        .apply_command(1, &pass())
        .expect("the creature resolves");
    assert_eq!(no_trigger.state.pending_triggers.len(), 0);
    assert_eq!(no_trigger.state.stack.len(), 0);
}

#[test]
fn issue_misc5_ashioks_reaper_draws_for_a_departing_enchantment() {
    let mut e = engine(729_004);
    inject_creature_on_battlefield(&mut e, 0, "ashioks_reaper");
    let enchantment = inject_permanent_on_battlefield(&mut e, 0, "rakish_crew");
    inject_card_into_hand(&mut e, 0, "disenchant");
    grant_pool(&mut e, 0);
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "disenchant");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(enchantment)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "Disenchant leaves hand and the Reaper draws one"
    );

    // A nonenchantment permanent leaving does not trigger the watcher.
    let mut no_draw = engine(729_014);
    inject_creature_on_battlefield(&mut no_draw, 0, "ashioks_reaper");
    let artifact = inject_permanent_on_battlefield(&mut no_draw, 0, "swiftfoot_boots");
    inject_card_into_hand(&mut no_draw, 0, "disenchant");
    grant_pool(&mut no_draw, 0);
    let hand_before = no_draw.state.players[0].hand.len();
    let slot = hand_index_for_card(&no_draw, 0, "disenchant");
    semantic::accepted(&mut no_draw, 0, &cast_spell(slot, target_object(artifact)));
    resolve_entire_stack_two_player(&mut no_draw);
    assert_eq!(no_draw.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(no_draw.state.players[0].hand.len(), hand_before - 1);
}

#[test]
fn issue_misc5_battlesong_berserker_pumps_and_grants_menace() {
    let mut e = GameEngine::new(729_005, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let berserker = inject_creature_on_battlefield(&mut e, 0, "battlesong_berserker");
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![berserker]))
        .expect("declare the Berserker as an attacker");
    assert_eq!(e.state.pending_triggers.len(), 1, "one attack, one trigger");
    assert!(
        e.apply_command(0, &choose_trigger_target(opponent))
            .is_err(),
        "an opponent creature is outside the You filter"
    );
    semantic::accepted(&mut e, 0, &choose_trigger_target(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(3), "+1/+0");
    assert_eq!(
        e.effective_toughness(target),
        Some(2),
        "toughness unchanged"
    );
    assert!(e.effective_has_keyword(target, Keyword::Menace));
    assert_eq!(e.effective_power(opponent), Some(2));
}

#[test]
fn issue_misc5_clammy_prowler_makes_another_attacker_unblockable() {
    let mut e = GameEngine::new(729_007, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let prowler = inject_creature_on_battlefield(&mut e, 0, "clammy_prowler");
    let fellow = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bench = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![prowler, fellow]))
        .expect("declare two attackers");
    assert_eq!(e.state.pending_triggers.len(), 1);
    assert!(
        e.apply_command(0, &choose_trigger_target(prowler)).is_err(),
        "the source is not 'another' creature"
    );
    assert!(
        e.apply_command(0, &choose_trigger_target(bench)).is_err(),
        "a nonattacking controlled creature is not a legal target"
    );
    assert!(
        e.apply_command(0, &choose_trigger_target(opponent))
            .is_err(),
        "a nonattacking opponent creature is not a legal target; the filter is not controller-scoped"
    );
    semantic::accepted(&mut e, 0, &choose_trigger_target(fellow));
    resolve_entire_stack_two_player(&mut e);
    assert!(
        zone_view_rules_annotation_labels(&mut e, 0, fellow)
            .iter()
            .any(|label| label == "Can't be blocked"),
        "the resolved restriction is published"
    );
    assert!(
        zone_view_rules_annotation_labels(&mut e, 0, prowler).is_empty(),
        "the source itself is not restricted"
    );
}

#[test]
fn issue_misc5_bake_into_a_pie_destroys_and_creates_food() {
    let mut e = engine(729_008);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "bake_into_a_pie");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "bake_into_a_pie");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(bear)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(
        battlefield_token_oids(&e, 0, "food").len(),
        1,
        "the Food token is created for the caster"
    );

    // A noncreature permanent is not a legal target.
    let mut bad = engine(729_018);
    let land = inject_permanent_on_battlefield(&mut bad, 1, "forest");
    inject_card_into_hand(&mut bad, 0, "bake_into_a_pie");
    grant_pool(&mut bad, 0);
    let slot = hand_index_for_card(&bad, 0, "bake_into_a_pie");
    assert!(
        bad.apply_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "a land is not a creature"
    );
}
