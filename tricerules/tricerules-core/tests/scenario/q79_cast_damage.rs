//! Actual-card scenarios for the queue-v79 02-cast-damage cohort.
//!
//! Expectations are printed Oracle values. CR 601.2 casts the spell; CR 603.2/603.3 put the
//! cast trigger above it; CR 120.3 deals the printed damage to each opponent. The pinned
//! 2018-10-05, 2023-06-16 and 2025-06-06 rulings confirm the trigger resolves before the spell
//! that caused it and even if that spell is countered.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn cast_divination(e: &mut GameEngine) -> u32 {
    inject_card_into_hand(e, 0, "divination");
    give_mana(
        e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "divination");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Divination");
    e.state
        .stack
        .iter()
        .find(|item| {
            e.state
                .objects
                .get(&item.id)
                .is_some_and(|object| object.card_id == "divination")
        })
        .expect("Divination on the stack")
        .id
}

fn cast_bears(e: &mut GameEngine) {
    inject_card_into_hand(e, 0, "grizzly_bears");
    give_mana(
        e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Grizzly Bears");
}

#[test]
fn q79_cast_damage_canary_simplest_kessig_hits_before_the_spell_resolves() {
    let mut e = engine(79_200_001);
    inject_creature_on_battlefield(&mut e, 0, "kessig_flamebreather");
    let hand_before = e.state.players[0].hand.len();
    let library_before = e.state.players[0].library.len();
    let spell = cast_divination(&mut e);
    assert_eq!(
        e.state.stack.len(),
        2,
        "spell and cast trigger are on the stack"
    );
    e.apply_scenario_command(0, &pass()).expect("active passes");
    e.apply_scenario_command(1, &pass())
        .expect("opponent passes");
    assert_eq!(
        e.state.players[1].life, 19,
        "the cast trigger resolves before Divination"
    );
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Stack,
        "Divination is still on the stack"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19, "one damage total");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 2,
        "the injected spell leaves hand and two cards are drawn"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 2,
        "Divination draws two"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let mut no_trigger = engine(79_200_002);
    inject_creature_on_battlefield(&mut no_trigger, 0, "kessig_flamebreather");
    cast_bears(&mut no_trigger);
    resolve_entire_stack_two_player(&mut no_trigger);
    assert_eq!(
        no_trigger.state.players[1].life, 20,
        "a creature spell does not trigger Kessig"
    );
}

#[test]
fn q79_cast_damage_canary_hardest_black_waltz_deals_two_and_keeps_keywords() {
    let mut e = engine(79_200_011);
    let waltz = inject_creature_on_battlefield(&mut e, 0, "black_waltz_no._3");
    assert!(
        e.effective_has_keyword(waltz, tricerules_cards::primitives::Keyword::Flying),
        "Black Waltz No. 3 has flying"
    );
    assert!(
        e.effective_has_keyword(waltz, tricerules_cards::primitives::Keyword::Deathtouch),
        "Black Waltz No. 3 has deathtouch"
    );
    let spell = cast_divination(&mut e);
    e.apply_scenario_command(0, &pass()).expect("active passes");
    e.apply_scenario_command(1, &pass())
        .expect("opponent passes");
    assert_eq!(
        e.state.players[1].life, 18,
        "the trigger deals 2 damage before Divination resolves"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Stack);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 18);
}

#[test]
fn q79_cast_damage_cards_apply_their_printed_damage() {
    for (index, (id, damage)) in [
        ("erebor_flamesmith", 1),
        ("electrostatic_field", 1),
        ("kessig_flamebreather", 1),
        ("black_waltz_no._3", 2),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(79_201_000 + index as u64);
        inject_creature_on_battlefield(&mut e, 0, id);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        cast_divination(&mut e);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - damage,
            "{id}: printed damage to each opponent"
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 2,
            "{id}: the injected spell leaves hand and two cards are drawn"
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 2,
            "{id}: Divination still resolves and draws two"
        );
    }
}

#[test]
fn q79_cast_damage_cards_ignore_creature_spells() {
    for (index, id) in [
        "erebor_flamesmith",
        "electrostatic_field",
        "kessig_flamebreather",
        "black_waltz_no._3",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(79_202_000 + index as u64);
        inject_creature_on_battlefield(&mut e, 0, id);
        cast_bears(&mut e);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[1].life, 20, "{id}: creature spell ignored");
    }
}

#[test]
fn q79_cast_damage_black_waltz_trigger_survives_a_countered_spell() {
    let mut e = engine(79_203_001);
    inject_creature_on_battlefield(&mut e, 0, "black_waltz_no._3");
    let hand_before = e.state.players[0].hand.len();
    let spell = cast_divination(&mut e);
    // The opponent counters Divination after the cast trigger is already on the stack.
    inject_card_into_hand(&mut e, 1, "counterspell");
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "counterspell");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes priority");
    e.apply_scenario_command(1, &cast_spell(slot, target_object(spell)))
        .expect("counter Divination");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 18,
        "the cast trigger still deals 2 damage"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the countered Divination does not draw"
    );
}

#[test]
fn q79_cast_damage_noncreature_artifact_distinguishes_the_filters() {
    for (id, expected_damage) in [
        ("erebor_flamesmith", 0),
        ("electrostatic_field", 0),
        ("kessig_flamebreather", 1),
        ("black_waltz_no._3", 2),
    ] {
        let mut e = engine(79_204_000);
        inject_creature_on_battlefield(&mut e, 0, id);
        inject_card_into_hand(&mut e, 0, "darksteel_ingot");
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 3,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "darksteel_ingot");
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast Darksteel Ingot");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - expected_damage,
            "{id}: noncreature artifact spell filter"
        );
    }
}

#[test]
fn q79_cast_damage_does_not_trigger_on_an_opponents_cast() {
    let mut e = engine(79_205_001);
    inject_creature_on_battlefield(&mut e, 0, "kessig_flamebreather");
    e.apply_scenario_command(0, &pass()).expect("active passes");
    inject_card_into_hand(&mut e, 1, "lightning_bolt");
    give_mana(
        &mut e,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_scenario_command(1, &cast_spell(slot, target_player(0)))
        .expect("opponent casts Lightning Bolt");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 17, "the Bolt resolves");
    assert_eq!(
        e.state.players[1].life, 20,
        "Kessig doesn't care about an opponent's cast"
    );
}
