//! Issue #351 — the eight reviewed pump, attack, activated, mass, and conditional-modifier
//! Standard cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 611.2a/514.2/701.26 govern the +2/+2 reach and untap pump and its cleanup expiry;
//! CR 611.2a/702.7 the +3/+0 reach-and-first-strike pump; CR 508.1/603.2/119.3 the
//! defending-player attack drain; CR 603.2/115.9b the Repartee targeted-spell cast trigger;
//! CR 602.1/611.2a/702.17 the mana-activated reach grant and its expiry; CR 602.1/509.1b/208
//! the power-bounded unblockable activation; CR 603.6a/611.3 the source-excluding mass -2/-2 and
//! its cleanup; and CR 604.1/611.3 the live control-an-artifact self modifier.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::BlockPair;

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

#[test]
fn issue_351_pillar_launch_pumps_reach_untaps_and_expires() {
    let mut engine = main1_engine(351_001, &["pillar_launch"], &[]);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&bear).expect("bear").tapped = true;
    ensure_in_hand(&mut engine, 0, "pillar_launch");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "pillar_launch");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Pillar Launch");
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "CR 611.2a: the pump applies only on resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(4));
    assert_eq!(engine.effective_toughness(bear), Some(4));
    assert!(engine.effective_has_keyword(bear, tricerules_cards::Keyword::Reach));
    assert!(!engine.state.objects[&bear].tapped, "CR 701.26: untap it");

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "CR 514.2: the pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(bear), Some(2));
    assert!(
        !engine.effective_has_keyword(bear, tricerules_cards::Keyword::Reach),
        "the until-end-of-turn reach grant expires at cleanup"
    );
}

#[test]
fn issue_351_smaugs_fury_pumps_reach_and_first_strike() {
    let mut engine = main1_engine(351_002, &["smaugs_fury"], &[]);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "smaugs_fury");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "smaugs_fury");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Smaug's Fury");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(5));
    assert_eq!(
        engine.effective_toughness(bear),
        Some(2),
        "CR 611.2a: the +3/+0 leaves toughness unchanged"
    );
    assert!(engine.effective_has_keyword(bear, tricerules_cards::Keyword::Reach));
    assert!(engine.effective_has_keyword(bear, tricerules_cards::Keyword::FirstStrike));

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(bear), Some(2));
    assert!(
        !engine.effective_has_keyword(bear, tricerules_cards::Keyword::FirstStrike),
        "the until-end-of-turn first strike expires at cleanup"
    );
}

#[test]
fn issue_351_agate_blade_assassin_drains_the_defending_player() {
    let mut engine = main1_engine(351_003, &["agate-blade_assassin"], &[]);
    let assassin = inject_creature_on_battlefield(&mut engine, 0, "agate-blade_assassin");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![assassin]))
        .expect("attack with Agate-Blade Assassin");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the attack trigger is on the stack"
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.players[1].life, 19,
        "CR 119.3: the defending player loses one life"
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "the attacker's controller gains one life"
    );
}

#[test]
fn issue_351_lecturing_scornmage_counts_only_targeted_instant_or_sorcery() {
    let mut engine = main1_engine(
        351_004,
        &["lecturing_scornmage", "titanic_growth", "opt"],
        &[],
    );
    let mage = move_ready_to_battlefield(&mut engine, 0, "lecturing_scornmage");
    let other = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    ensure_card_in_hand(&mut engine, 0, "titanic_growth");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "titanic_growth");
    engine
        .apply_command(0, &cast_spell(slot, target_object(other)))
        .expect("cast a creature-targeting instant");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "CR 603.2/115.9b: Repartee puts a trigger on the stack above the spell"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&mage].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the counter lands on the source"
    );

    ensure_card_in_hand(&mut engine, 0, "opt");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "opt");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast a non-targeting instant");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "an instant without a creature target does not satisfy Repartee"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&mage].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the non-targeting cast adds no counter"
    );
}

#[test]
fn issue_351_frog_butler_gains_reach_and_expires() {
    let mut engine = main1_engine(351_005, &["frog_butler"], &[]);
    let frog = move_ready_to_battlefield(&mut engine, 0, "frog_butler");
    assert!(
        !engine.effective_has_keyword(frog, tricerules_cards::Keyword::Reach),
        "Frog Butler has no printed reach"
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let batch =
        apply_ability(&mut engine, 0, frog, 1, Vec::new()).expect("activate the reach grant");
    assert_eq!(
        life_changes_in(&batch).len(),
        0,
        "the activation has no life effect"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(frog, tricerules_cards::Keyword::Reach));

    end_active_turn(&mut engine, 0);
    assert!(
        !engine.effective_has_keyword(frog, tricerules_cards::Keyword::Reach),
        "CR 514.2: the reach grant expires at cleanup"
    );
}

#[test]
fn issue_351_ragged_playmate_makes_a_small_creature_unblockable() {
    let mut engine = main1_engine(
        351_006,
        &["ragged_playmate"],
        &["grizzly_bears", "hill_giant"],
    );
    let playmate = move_ready_to_battlefield(&mut engine, 0, "ragged_playmate");
    let small = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let big = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, playmate, 0, target_object(small))
        .expect("activate the unblockable grant");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, small)
            .contains(&"Can't be blocked".into()),
        "CR 509.1b: the small creature is unblockable"
    );

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![small]))
        .expect("attack with the small creature");
    let legal = engine.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .clone();
    assert!(
        legal.iter().all(|pair| pair.attacker_id != small),
        "CR 509.1b: no blocker may be declared against the unblockable attacker"
    );
    assert!(
        engine
            .apply_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: small,
                    blocker_id: big,
                }]),
            )
            .is_err(),
        "a legal blocker cannot block an unblockable creature"
    );
}

#[test]
fn issue_351_ragged_playmate_rejects_a_power_three_target() {
    let mut engine = main1_engine(351_007, &["ragged_playmate", "titanic_growth"], &[]);
    let playmate = move_ready_to_battlefield(&mut engine, 0, "ragged_playmate");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "titanic_growth");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "titanic_growth");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("pump the target to power six");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(target), Some(6));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    assert!(
        apply_ability(&mut engine, 0, playmate, 0, target_object(target)).is_err(),
        "CR 208: a power-6 creature is not a legal target"
    );
}

#[test]
fn issue_351_shefet_archfiend_gives_all_other_creatures_minus_two() {
    let mut engine = main1_engine(351_008, &["shefet_archfiend"], &[]);
    let archfiend = move_ready_to_battlefield(&mut engine, 0, "shefet_archfiend");
    let own_big = inject_creature_with_stats(&mut engine, 0, "hill_giant", 3, 3);
    let own_larger = inject_creature_with_stats(&mut engine, 0, "craw_wurm", 6, 4);
    let opposing = inject_creature_with_stats(&mut engine, 1, "hill_giant", 3, 3);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.effective_power(archfiend),
        Some(5),
        "CR 611.3: the source is excluded from its own sweep"
    );
    assert_eq!(engine.effective_toughness(archfiend), Some(5));
    assert_eq!(engine.effective_power(own_big), Some(1));
    assert_eq!(engine.effective_toughness(own_big), Some(1));
    assert_eq!(
        engine.effective_power(own_larger),
        Some(4),
        "a 6/4 becomes 4/2"
    );
    assert_eq!(engine.effective_toughness(own_larger), Some(2));
    assert_eq!(
        engine.effective_power(opposing),
        Some(1),
        "the sweep is player-set-generic and hits every other creature"
    );
    assert_eq!(engine.effective_toughness(opposing), Some(1));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(own_big),
        Some(3),
        "CR 514.2: the mass pump expires at cleanup"
    );
    assert_eq!(engine.effective_power(own_larger), Some(6));
    assert_eq!(engine.effective_toughness(own_larger), Some(4));
    assert_eq!(engine.effective_power(opposing), Some(3));
}

#[test]
fn issue_351_shefet_archfiend_sweep_kills_a_two_toughness_creature() {
    let mut engine = main1_engine(351_011, &["shefet_archfiend"], &[]);
    let fragile = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut engine, 0, "shefet_archfiend");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&fragile].zone,
        Zone::Graveyard,
        "CR 704.5f: a 2/2 hit by -2/-2 dies as a state-based action"
    );
}

#[test]
fn issue_351_gravblade_heavy_tracks_artifacts_live() {
    let mut engine = main1_engine(351_009, &["gravblade_heavy"], &[]);
    let heavy = move_ready_to_battlefield(&mut engine, 0, "gravblade_heavy");
    assert_eq!(
        engine.effective_power(heavy),
        Some(3),
        "no artifact, no bonus"
    );
    assert!(
        !engine.effective_has_keyword(heavy, tricerules_cards::Keyword::Deathtouch),
        "no artifact, no deathtouch"
    );

    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "prophetic_prism");
    assert_eq!(
        engine.effective_power(heavy),
        Some(4),
        "CR 604.1/611.3: controlling an artifact grants +1/+0"
    );
    assert!(
        engine.effective_has_keyword(heavy, tricerules_cards::Keyword::Deathtouch),
        "controlling an artifact grants deathtouch"
    );

    engine
        .state
        .objects
        .get_mut(&artifact)
        .expect("artifact")
        .zone = Zone::Graveyard;
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != artifact);
    assert_eq!(
        engine.effective_power(heavy),
        Some(3),
        "the conditional modifier reevaluates when the artifact leaves"
    );
    assert!(
        !engine.effective_has_keyword(heavy, tricerules_cards::Keyword::Deathtouch),
        "the deathtouch grant drops with the artifact"
    );
}

#[test]
fn issue_351_scenarios_reach_main1() {
    let engine = main1_engine(351_010, &[], &[]);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}
