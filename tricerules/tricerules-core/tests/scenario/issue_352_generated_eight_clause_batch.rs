//! Issue #352 — the eight reviewed pump, destroy, activation, trigger, and replacement Standard
//! cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 611.2a/514.2/702.7 govern the +2/+2 first-strike pump and its cleanup expiry; CR 701.8/115.1
//! the creature-or-Vehicle destroy and its target legality; CR 602.2/509.1b the mana-activated
//! unblockable restriction and its expiry; CR 508.1/603.2/119.3 the self-attack lifegain; CR
//! 602.2/701.9/701.21 the discard-and-sacrifice draw; CR 602.2/701.9/702.12/701.26 the
//! discard-to-grant-indestructible-and-tap activation; CR 603.6a/611.2a the Alliance
//! source-excluding pump; and CR 614.1c/122.6 the conditional entry counter.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
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
fn issue_352_interjection_pumps_first_strike_and_expires() {
    let mut engine = main1_engine(352_001, &["interjection"], &[]);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "interjection");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "interjection");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Interjection");
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "CR 611.2a: the pump applies only on resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(4));
    assert_eq!(engine.effective_toughness(bear), Some(4));
    assert!(engine.effective_has_keyword(bear, Keyword::FirstStrike));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "CR 514.2: the pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(bear), Some(2));
    assert!(
        !engine.effective_has_keyword(bear, Keyword::FirstStrike),
        "the until-end-of-turn first strike expires at cleanup"
    );
}

#[test]
fn issue_352_spin_out_destroys_a_creature_and_a_vehicle_but_not_an_artifact() {
    let mut engine = main1_engine(352_002, &["spin_out", "spin_out"], &[]);
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let wagon = inject_permanent_on_battlefield(&mut engine, 1, "lumbering_worldwagon");
    let prism = inject_permanent_on_battlefield(&mut engine, 1, "prophetic_prism");

    // A non-Vehicle artifact is not a legal target for the union.
    ensure_in_hand(&mut engine, 0, "spin_out");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "spin_out");
    assert!(
        engine
            .apply_command(0, &cast_spell(slot, target_object(prism)))
            .is_err(),
        "CR 115.1: an artifact that is not a Vehicle is not a legal target"
    );

    // A Vehicle permanent is.
    engine
        .apply_command(0, &cast_spell(slot, target_object(wagon)))
        .expect("cast Spin Out at the Vehicle");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&wagon].zone,
        Zone::Graveyard,
        "CR 701.8: the Vehicle is destroyed"
    );

    // A creature is.
    ensure_in_hand(&mut engine, 0, "spin_out");
    let slot = hand_index_for_card(&engine, 0, "spin_out");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Spin Out at the creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&prism].zone,
        Zone::Battlefield,
        "the rejected cast never affected the artifact"
    );
}

#[test]
fn issue_352_elvenkings_harper_grants_unblockable_until_cleanup() {
    let mut engine = main1_engine(352_003, &["elvenkings_harper"], &[]);
    let harper = move_ready_to_battlefield(&mut engine, 0, "elvenkings_harper");
    let small = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, harper, 0, target_object(small))
        .expect("activate the unblockable grant");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, small)
            .contains(&"Can't be blocked".into()),
        "CR 509.1b: the target is unblockable"
    );

    end_active_turn(&mut engine, 0);
    assert!(
        !zone_view_rules_annotation_labels(&mut engine, 0, small)
            .contains(&"Can't be blocked".into()),
        "CR 514.2: the block restriction expires at cleanup"
    );
}

#[test]
fn issue_352_elvenkings_harper_forbids_declaring_a_blocker() {
    let mut engine = main1_engine(352_011, &["elvenkings_harper"], &["grizzly_bears"]);
    let harper = move_ready_to_battlefield(&mut engine, 0, "elvenkings_harper");
    let small = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let big = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, harper, 0, target_object(small))
        .expect("activate the unblockable grant");
    resolve_entire_stack_two_player(&mut engine);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![small]))
        .expect("attack with the unblockable creature");
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
fn issue_352_moonrise_cleric_gains_life_on_attack() {
    let mut engine = main1_engine(352_004, &["moonrise_cleric"], &[]);
    let cleric = inject_creature_on_battlefield(&mut engine, 0, "moonrise_cleric");
    assert_eq!(engine.state.players[0].life, 20);
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![cleric]))
        .expect("attack with Moonrise Cleric");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the attack trigger is on the stack"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 21,
        "CR 119.3: the controller gains one life"
    );
}

#[test]
fn issue_352_masked_meower_discards_and_sacrifices_to_draw() {
    let mut engine = main1_engine(352_005, &["masked_meower", "grizzly_bears"], &[]);
    let meower = relocate_to_battlefield(&mut engine, 0, "masked_meower", false);
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    let discarded_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let discarded = engine.state.players[0].hand[discarded_slot];
    let library_before = engine.state.players[0].library.len();

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                meower,
                0,
                Vec::new(),
                vec![hand_cost_selection(0, discarded_slot as u32)],
            ),
        )
        .expect("discard a card and sacrifice Masked Meower");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&discarded].zone,
        Zone::Graveyard,
        "CR 701.9: the discarded card is in the graveyard"
    );
    assert_eq!(
        engine.state.objects[&meower].zone,
        Zone::Graveyard,
        "CR 701.21: the source was sacrificed"
    );
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "the ability drew a card"
    );
}

#[test]
fn issue_352_iron_shield_elf_discards_for_indestructible_and_taps() {
    let mut engine = main1_engine(352_006, &["iron-shield_elf", "grizzly_bears"], &[]);
    let elf = relocate_to_battlefield(&mut engine, 0, "iron-shield_elf", false);
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    let discarded_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let discarded = engine.state.players[0].hand[discarded_slot];
    assert!(!engine.state.objects[&elf].tapped);

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                elf,
                0,
                Vec::new(),
                vec![hand_cost_selection(0, discarded_slot as u32)],
            ),
        )
        .expect("discard a card to grant indestructible");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert!(
        engine.effective_has_keyword(elf, Keyword::Indestructible),
        "CR 702.12: the source gains indestructible"
    );
    assert!(
        engine.state.objects[&elf].tapped,
        "CR 701.26: the source tapped"
    );
}

#[test]
fn issue_352_east_wind_avatar_pumps_on_another_creature_only() {
    let mut engine = main1_engine(
        352_007,
        &["east_wind_avatar", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let avatar = move_ready_to_battlefield(&mut engine, 0, "east_wind_avatar");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(avatar),
        Some(2),
        "CR 603.6a: the source's own entry is excluded"
    );

    move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(avatar),
        Some(3),
        "another creature you control entering pumps +1/+0"
    );
    assert_eq!(engine.effective_toughness(avatar), Some(4));

    move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(avatar),
        Some(3),
        "an opponent's creature is outside the controller scope"
    );

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(avatar),
        Some(2),
        "CR 514.2: the Alliance pump expires at cleanup"
    );
}

#[test]
fn issue_352_cackling_slasher_enters_with_a_counter_only_after_a_death() {
    let mut no_death = main1_engine(352_008, &["cackling_slasher"], &[]);
    let slasher = move_ready_to_battlefield(&mut no_death, 0, "cackling_slasher");
    resolve_entire_stack_two_player(&mut no_death);
    assert_eq!(
        no_death.state.objects[&slasher].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "no creature died this turn"
    );

    let decks = Some(vec![
        deck_with("swamp", &["cackling_slasher", "murder"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut with_death = GameEngine::new(352_009, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut with_death);
    let bear = relocate_to_battlefield(&mut with_death, 1, "grizzly_bears", false);
    ensure_in_hand(&mut with_death, 0, "murder");
    grant_pool(&mut with_death, 0);
    let murder = hand_index_for_card(&with_death, 0, "murder");
    with_death
        .apply_command(0, &cast_spell(murder, target_object(bear)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(&mut with_death);
    assert_eq!(with_death.state.turn_history.current.creatures_died, 1);

    let slasher = move_ready_to_battlefield(&mut with_death, 0, "cackling_slasher");
    resolve_entire_stack_two_player(&mut with_death);
    assert_eq!(
        with_death.state.objects[&slasher].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "CR 614.1c/122.6: the entry replacement adds a +1/+1 counter"
    );
    assert_eq!(with_death.effective_power(slasher), Some(4));
    assert_eq!(with_death.effective_toughness(slasher), Some(4));
}

#[test]
fn issue_352_scenarios_reach_main1() {
    let engine = main1_engine(352_010, &[], &[]);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}
