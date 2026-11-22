//! Actual-card scenarios for the queue-v54 batch C cohorts.
//!
//! Expectations are printed Oracle values. CR 301.5/702.6 (Equipment and equip), CR 613.4c
//! attached modifiers, CR 510.2 combat damage, CR 111.10a Treasure, CR 611.2a/514.2 pump
//! duration, CR 702.29 cycling, CR 603.6c dies triggers, CR 101.4/701.21a edicts and
//! CR 115.1 targeting.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cycle_from_hand(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

fn engine(seed: u64, p0: &[&str], p1: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("island", p0), deck_with("forest", p1)]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

// ---------------------------------------------------------------------------
// 07-equipment-pump

#[test]
fn q54_canary_simplest_darksteel_axe_equips_for_two_and_pumps_two() {
    let mut e = engine(54_070_001, &["darksteel_axe"], &["grizzly_bears"]);
    inject_card_into_hand(&mut e, 0, "darksteel_axe");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "darksteel_axe");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Darksteel Axe");
    resolve_entire_stack_two_player(&mut e);
    let axe = battlefield_object_for_card(&e, 0, "darksteel_axe");
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, axe, 0, target_object(bear)).expect("equip");
    pass_both_players(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{2}} equip cost");
    assert_eq!(
        e.state.objects[&axe].attached_to,
        Some(AttachmentRecipient::Object(bear)),
        "the Equipment is attached after equip resolves"
    );
    assert_eq!(e.effective_power(bear), Some(4), "+2/+0 from the Equipment");
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(e.effective_has_keyword(axe, Keyword::Indestructible));
    assert!(
        !e.effective_has_keyword(bear, Keyword::Indestructible),
        "indestructible is on the Equipment only"
    );
}

#[test]
fn q54_canary_hardest_prying_blade_makes_a_treasure_on_combat_damage() {
    let decks = Some(vec![
        deck_with("island", &["prying_blade"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        54_070_002,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    inject_card_into_hand(&mut e, 0, "prying_blade");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "prying_blade");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Prying Blade");
    resolve_entire_stack_two_player(&mut e);
    let blade = battlefield_object_for_card(&e, 0, "prying_blade");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, blade, 0, target_object(bear)).expect("equip");
    pass_both_players(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{2}} equip cost");
    assert_eq!(e.effective_power(bear), Some(3), "+1/+0 from the Equipment");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap pass begin combat");
    e.apply_scenario_command(0, &declare_attackers(vec![bear]))
        .expect("declare equipped attacker");
    e.apply_scenario_command(0, &pass()).expect("attacker pass");
    e.apply_scenario_command(1, &pass()).expect("defender pass");
    e.apply_scenario_command(0, &pass())
        .expect("attacker blockers pass");
    e.apply_scenario_command(1, &pass())
        .expect("defender blockers pass and combat damage");
    assert_eq!(
        e.state.stack.len(),
        1,
        "Prying Blade trigger reaches the stack"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert_eq!(battlefield_token_oids(&e, 1, "treasure").len(), 0);
}

#[test]
fn q54_no_dachi_grants_two_power_and_first_strike_for_three() {
    let mut e = engine(54_071_001, &["no-dachi"], &["grizzly_bears"]);
    inject_card_into_hand(&mut e, 0, "no-dachi");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "no-dachi");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast No-Dachi");
    resolve_entire_stack_two_player(&mut e);
    let sword = battlefield_object_for_card(&e, 0, "no-dachi");
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, sword, 0, target_object(bear)).expect("equip");
    pass_both_players(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{3}} equip cost");
    assert_eq!(e.effective_power(bear), Some(4));
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(e.effective_has_keyword(bear, Keyword::FirstStrike));
}

#[test]
fn q54_equipment_rejects_an_opponents_creature() {
    let mut e = engine(54_072_001, &["darksteel_axe"], &["grizzly_bears"]);
    let axe = relocate_to_battlefield(&mut e, 0, "darksteel_axe", false);
    let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    assert!(
        apply_ability(&mut e, 0, axe, 0, target_object(other)).is_err(),
        "equip targets only a creature you control"
    );
    assert_eq!(e.state.players[0].mana_pool.colorless, 2);
    assert!(e.state.objects[&axe].attached_to.is_none());
}

#[test]
fn q54_equipment_fizzles_when_the_equip_target_left() {
    let mut e = engine(
        54_073_001,
        &["darksteel_axe", "doom_blade"],
        &["grizzly_bears"],
    );
    inject_card_into_hand(&mut e, 0, "darksteel_axe");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "darksteel_axe");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Darksteel Axe");
    resolve_entire_stack_two_player(&mut e);
    let axe = battlefield_object_for_card(&e, 0, "darksteel_axe");
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, axe, 0, target_object(bear)).expect("equip");
    inject_card_into_hand(&mut e, 0, "doom_blade");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let doom = hand_index_for_card(&e, 0, "doom_blade");
    e.apply_scenario_command(0, &cast_spell(doom, target_object(bear)))
        .expect("destroy the equip target in response");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert!(e.state.objects[&axe].attached_to.is_none());
    assert_eq!(e.state.objects[&axe].zone, Zone::Battlefield);
}

// ---------------------------------------------------------------------------
// 08-pump-spell-riders

#[test]
fn q54_canary_simplest_spatial_contortion_three_minus_three_kills_a_bear() {
    let mut e = engine(54_080_001, &["spatial_contortion"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "spatial_contortion");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "spatial_contortion");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Spatial Contortion");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{1}}{{C}} cost");
    assert_eq!(
        e.state.objects[&bear].zone,
        Zone::Graveyard,
        "a 2/2 with -3 toughness dies to state-based actions"
    );
}

#[test]
fn q54_canary_hardest_sokkas_sword_training_pumps_and_makes_a_clue() {
    let mut e = engine(54_080_002, &["sokkas_sword_training"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "sokkas_sword_training");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "sokkas_sword_training");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Sokka's Sword Training");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{1}}{{W}} cost");
    assert_eq!(e.effective_power(bear), Some(4));
    assert_eq!(e.effective_toughness(bear), Some(4));
    assert_eq!(
        battlefield_token_oids(&e, 0, "clue").len(),
        1,
        "one Clue token under the caster"
    );
    assert_eq!(battlefield_token_oids(&e, 1, "clue").len(), 0);
}

#[test]
fn q54_iron_will_pumps_zero_four_and_cycles_for_two() {
    let mut e = engine(54_081_001, &["iron_will"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "iron_will");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "iron_will");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Iron Will");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{W}} cost");
    assert_eq!(e.effective_power(bear), Some(2));
    assert_eq!(e.effective_toughness(bear), Some(6));

    let mut e = engine(54_081_002, &["iron_will"], &["grizzly_bears"]);
    inject_card_into_hand(&mut e, 0, "iron_will");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "iron_will");
    let source = e.state.players[0].hand[slot];
    let hand = e.state.players[0].hand.len();
    let library = e.state.players[0].library.len();
    e.apply_scenario_command(0, &cycle_from_hand(&e, source))
        .expect("cycle Iron Will");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand, "discard one, draw one");
    assert_eq!(e.state.players[0].library.len(), library - 1);
    assert_eq!(count_card_id_in_graveyard(&e, 0, "iron_will"), 1);
}

#[test]
fn q54_spatial_contortion_rejects_a_player_target() {
    let mut e = engine(54_082_001, &["spatial_contortion"], &["grizzly_bears"]);
    inject_card_into_hand(&mut e, 0, "spatial_contortion");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "spatial_contortion");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .is_err(),
        "a player is not a legal creature target"
    );
    assert_eq!(e.state.players[0].mana_pool.colorless, 2);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q54_iron_will_pump_expires_at_cleanup() {
    let mut e = engine(54_083_001, &["iron_will"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "iron_will");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "iron_will");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Iron Will");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_toughness(bear), Some(6));
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_toughness(bear), Some(2));
}

// ---------------------------------------------------------------------------
// 09-each-player-sacrifices

#[test]
fn q54_canary_simplest_innocent_blood_makes_each_player_sacrifice() {
    let mut e = engine(54_090_001, &["innocent_blood"], &["grizzly_bears"]);
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let own_kept = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let other = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let other_kept = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    inject_card_into_hand(&mut e, 0, "innocent_blood");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "innocent_blood");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Innocent Blood");
    pass_priority_round(&mut e);
    let deciding = e
        .state
        .pending_resolution
        .as_ref()
        .expect("first choice")
        .deciding_player;
    let seat = e.state.player_idx(deciding).expect("known deciding player");
    assert_eq!(seat, 0, "the active player chooses first");
    e.apply_scenario_command(deciding, &submit_resolution_choice(vec![own]))
        .expect("the active player chooses its own creature");
    assert_eq!(
        e.state.objects[&own].zone,
        Zone::Battlefield,
        "nothing is sacrificed until every player has chosen"
    );
    let deciding = e
        .state
        .pending_resolution
        .as_ref()
        .expect("second choice")
        .deciding_player;
    let seat = e.state.player_idx(deciding).expect("known deciding player");
    assert_eq!(seat, 1, "then the other player chooses");
    e.apply_scenario_command(deciding, &submit_resolution_choice(vec![other]))
        .expect("the second player chooses its own creature");
    assert_eq!(e.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&other].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&own_kept].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&other_kept].zone, Zone::Battlefield);
}

#[test]
fn q54_demons_disciple_can_sacrifice_a_planeswalker() {
    let mut e = engine(54_090_003, &["demons_disciple"], &["jace_beleren"]);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let jace = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
    e.state
        .objects
        .get_mut(&jace)
        .expect("Jace")
        .set_counter(CounterKind::Loyalty, 3);
    inject_card_into_hand(&mut e, 0, "demons_disciple");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "demons_disciple");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Demon's Disciple");
    pass_priority_round(&mut e);
    pass_priority_round(&mut e);
    while e.state.pending_resolution.is_some() {
        let deciding = e.state.pending_resolution.as_ref().unwrap().deciding_player;
        let seat = e.state.player_idx(deciding).expect("known deciding player");
        let victim = if seat == 0 { bear } else { jace };
        let player = e.state.players[seat].id;
        e.apply_scenario_command(player, &submit_resolution_choice(vec![victim]))
            .expect("choose the sacrificed permanent");
    }
    assert_eq!(
        e.state.objects[&jace].zone,
        Zone::Graveyard,
        "the planeswalker branch is a legal sacrifice choice"
    );
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&battlefield_object_for_card(&e, 0, "demons_disciple")].zone,
        Zone::Battlefield,
        "the Disciple survives when its controller chooses another creature"
    );
}

#[test]
fn q54_canary_hardest_demons_disciple_can_sacrifice_itself() {
    let mut e = engine(54_090_002, &["demons_disciple"], &[]);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    inject_card_into_hand(&mut e, 0, "demons_disciple");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "demons_disciple");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Demon's Disciple");
    pass_priority_round(&mut e);
    pass_priority_round(&mut e);
    let source = battlefield_object_for_card(&e, 0, "demons_disciple");
    let deciding = e
        .state
        .pending_resolution
        .as_ref()
        .expect("choice")
        .deciding_player;
    let seat = e.state.player_idx(deciding).expect("known deciding player");
    let player = e.state.players[seat].id;
    e.apply_scenario_command(player, &submit_resolution_choice(vec![source]))
        .expect("the source may be sacrificed");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&bear].zone,
        Zone::Battlefield,
        "the other creature survived the real two-candidate choice"
    );
}

#[test]
fn q54_abyssal_gatekeeper_triggers_on_death() {
    let mut e = engine(
        54_091_001,
        &["abyssal_gatekeeper", "murder"],
        &["grizzly_bears"],
    );
    let gatekeeper = relocate_to_battlefield(&mut e, 0, "abyssal_gatekeeper", false);
    let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "murder");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let doom = hand_index_for_card(&e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(doom, target_object(gatekeeper)))
        .expect("destroy Abyssal Gatekeeper with Murder");
    resolve_entire_stack_two_player(&mut e);
    while e.state.pending_resolution.is_some() {
        let deciding = e.state.pending_resolution.as_ref().unwrap().deciding_player;
        let seat = e.state.player_idx(deciding).expect("known deciding player");
        let victim = if seat == 0 { gatekeeper } else { other };
        let player = e.state.players[seat].id;
        e.apply_scenario_command(player, &submit_resolution_choice(vec![victim]))
            .expect("choose the sacrificed creature");
    }
    assert_eq!(e.state.objects[&other].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&gatekeeper].zone, Zone::Graveyard);
}

#[test]
fn q54_innocent_blood_with_no_creatures_resolves_without_choices() {
    let mut e = engine(54_092_001, &["innocent_blood"], &[]);
    inject_card_into_hand(&mut e, 0, "innocent_blood");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "innocent_blood");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Innocent Blood");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
    assert_eq!(count_card_id_in_graveyard(&e, 0, "innocent_blood"), 1);
}
