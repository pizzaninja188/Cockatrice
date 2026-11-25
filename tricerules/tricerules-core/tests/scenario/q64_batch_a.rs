//! Actual-card scenarios for the queue-v64 batch A cohorts.
//!
//! Three stable typed shapes: an opponent-wide shrink plus draw or cycling, a conditional self
//! first strike during your turn with at most one activated pump, and an uncounterable creature
//! with one printed keyword. Expectations are printed Oracle values. CR 611.2c fixes the affected
//! set at resolution; CR 613.4c/514.2 apply and expire layer-7c modifiers; CR 702.29a-b makes
//! cycling a hand-zone activated ability; CR 702.7/611.3a govern the conditional first strike;
//! CR 113.6g/701.6 govern an uncounterable spell; CR 702.18a is shroud and CR 702.16 is
//! protection; and CR 121.1 draws a card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{AbilitySourceZone, ActivateAbility, RuledCommand};

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn main1_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn cast_named(
    e: &mut GameEngine,
    player: i32,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) -> u32 {
    inject_card_into_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    e.state.stack.last().expect("cast spell on stack").id
}

fn hand_ability(e: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
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
            ability_index,
            ..Default::default()
        })),
    }
}

fn cycle_from_hand(e: &mut GameEngine, card: &str) {
    let source = inject_card_into_hand(e, 0, card);
    let hand_before = e.state.players[0].hand.len();
    give_mana(
        e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(e, source, 0))
        .unwrap_or_else(|error| panic!("cycle {card}: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "{card} discarded"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 1,
        "{card}: cycling discards one and draws one"
    );
}

// ── 01 opponent shrink ─────────────────────────────────────────────────────

#[test]
fn q64_shrink_canary_simplest_suffocating_fumes_reduces_only_opposing_creatures() {
    let mut e = main1_engine(64_010_001);
    let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 1, 1);
    let spell = cast_named(
        &mut e,
        0,
        "suffocating_fumes",
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(mine), e.effective_toughness(mine)),
        (Some(2), Some(2)),
        "the caster's creature is unaffected"
    );
    assert_eq!(
        e.state.objects[&theirs].zone,
        Zone::Graveyard,
        "a 1/1 opposing creature dies to -1/-1"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "no mana remains");
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q64_shrink_canary_hardest_blinding_spray_draws_even_with_no_opposing_creatures() {
    let mut e = main1_engine(64_010_002);
    let hand_before = e.state.players[0].hand.len();
    let spell = cast_named(
        &mut e,
        0,
        "blinding_spray",
        ManaGift {
            u: 1,
            c: 4,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the spell leaves hand and the controller draws one"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q64_shrink_cards_apply_their_printed_values_and_do_not_touch_friendly_creatures() {
    for (index, (card, mana, shrink)) in [
        (
            "blinding_spray",
            ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
            (-4, 0),
        ),
        (
            "hampering_snare",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            (-2, 0),
        ),
        (
            "suffocating_fumes",
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            (-1, -1),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(64_011_000 + index as u64);
        let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        let first = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        let second = inject_creature_with_stats(&mut e, 1, "hill_giant", 5, 5);
        cast_named(&mut e, 0, card, gift(&mana), vec![]);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(mine), e.effective_toughness(mine)),
            (Some(5), Some(5)),
            "{card}: friendly creature unchanged"
        );
        for (id, creature) in [("first", first), ("second", second)] {
            assert_eq!(
                (e.effective_power(creature), e.effective_toughness(creature)),
                (Some((5 + shrink.0) as u32), Some((5 + shrink.1) as u32)),
                "{card}: exact printed shrink on {id} opposing creature"
            );
        }
    }
}

#[test]
fn q64_shrink_cards_do_not_affect_later_entrants_or_expire_at_cleanup() {
    let mut e = main1_engine(64_012_001);
    let existing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_named(
        &mut e,
        0,
        "blinding_spray",
        ManaGift {
            u: 1,
            c: 4,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(existing), e.effective_toughness(existing)),
        (Some(1), Some(5)),
        "the printed -4/-0 applied"
    );
    let late = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    assert_eq!(
        (e.effective_power(late), e.effective_toughness(late)),
        (Some(2), Some(2)),
        "a creature entering after resolution is unaffected"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(existing), e.effective_toughness(existing)),
        (Some(5), Some(5)),
        "CR 514.2 restores the printed body at cleanup"
    );
}

#[test]
fn q64_shrink_cards_cycle_for_two_and_draw_one() {
    for card in ["hampering_snare", "suffocating_fumes"] {
        let mut e = main1_engine(64_013_001);
        cycle_from_hand(&mut e, card);
    }
}

// ── 02 during-your-turn first strike ───────────────────────────────────────

#[test]
fn q64_first_strike_canary_simplest_pouncing_lynx_is_conditional() {
    let mut e = main1_engine(64_020_001);
    let lynx = cast_named(
        &mut e,
        0,
        "pouncing_lynx",
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&lynx].zone, Zone::Battlefield);
    assert!(e.effective_has_keyword(lynx, Keyword::FirstStrike));
    e.state.active_player_idx = 1;
    assert!(!e.effective_has_keyword(lynx, Keyword::FirstStrike));
    e.state.active_player_idx = 0;
    assert!(e.effective_has_keyword(lynx, Keyword::FirstStrike));
}

#[test]
fn q64_first_strike_canary_hardest_spartan_veteran_pumps_only_itself() {
    let mut e = main1_engine(64_020_002);
    let veteran = cast_named(
        &mut e,
        0,
        "spartan_veteran",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(veteran, Keyword::FirstStrike));
    let other = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(&e, veteran, 0, vec![]))
        .expect("activate Spartan Veteran");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(veteran), e.effective_toughness(veteran)),
        (Some(2), Some(1)),
        "the printed +1/+0 applies to the source"
    );
    assert_eq!(
        (e.effective_power(other), e.effective_toughness(other)),
        (Some(2), Some(2)),
        "another creature is untouched"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact activation cost");
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(veteran), e.effective_toughness(veteran)),
        (Some(1), Some(1)),
        "CR 514.2 restores the printed body"
    );
}

#[test]
fn q64_first_strike_cards_cast_with_their_printed_costs_and_bodies() {
    for (index, (card, mana, power, toughness, colored)) in [
        (
            "fresh-faced_recruit",
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            2,
            1,
            "red",
        ),
        (
            "fresh-faced_recruit",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            2,
            1,
            "white",
        ),
        (
            "pouncing_lynx",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            2,
            1,
            "white",
        ),
        (
            "spartan_veteran",
            ManaGift {
                r: 1,
                ..Default::default()
            },
            1,
            1,
            "red",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(64_021_000 + index as u64);
        let permanent = cast_named(&mut e, 0, card, gift(&mana), vec![]);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{card}: exact {colored} cost");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&permanent].zone, Zone::Battlefield);
        assert_eq!(
            (
                e.effective_power(permanent),
                e.effective_toughness(permanent)
            ),
            (Some(power), Some(toughness)),
            "{card}: printed body"
        );
        assert!(
            e.effective_has_keyword(permanent, Keyword::FirstStrike),
            "{card}: first strike during its controller's turn"
        );
        e.state.active_player_idx = 1;
        assert!(
            !e.effective_has_keyword(permanent, Keyword::FirstStrike),
            "{card}: no first strike during an opponent's turn"
        );
    }
}

// ── 03 uncounterable creatures ─────────────────────────────────────────────

#[test]
fn q64_uncounterable_canary_simplest_blurred_mongoose_resists_a_counter_and_has_shroud() {
    let mut e = main1_engine(64_030_001);
    let mongoose = cast_named(
        &mut e,
        0,
        "blurred_mongoose",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    let counterspell = cast_named(
        &mut e,
        1,
        "counterspell",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        target_object(mongoose),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&mongoose].zone,
        Zone::Battlefield,
        "the uncounterable creature spell resolves"
    );
    assert_eq!(e.state.objects[&counterspell].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
    // Shroud works on the battlefield: a red burn spell cannot target it.
    let hand_before = e.state.players[0].hand.len();
    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    let pool_before = pool(&e);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(mongoose)))
            .is_err(),
        "shroud rejects the target"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "no card spent"
    );
    assert_eq!(pool(&e), pool_before, "no mana spent");
}

#[test]
fn q64_uncounterable_canary_hardest_scragnoth_has_protection_from_blue() {
    let mut e = main1_engine(64_030_002);
    let scragnoth = cast_named(
        &mut e,
        0,
        "scragnoth",
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&scragnoth].zone, Zone::Battlefield);
    assert_eq!(
        (
            e.effective_power(scragnoth),
            e.effective_toughness(scragnoth)
        ),
        (Some(3), Some(4))
    );
    let hand_before = e.state.players[0].hand.len();
    inject_card_into_hand(&mut e, 0, "unsummon");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "unsummon");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(scragnoth)))
            .is_err(),
        "protection from blue rejects a blue target"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "no card spent"
    );
    assert_eq!(pool(&e), (0, 1, 0, 0, 0, 0), "no mana spent");
}

#[test]
fn q64_uncounterable_cards_all_resolve_through_a_counter_attempt() {
    for (index, (card, mana, power, toughness)) in [
        (
            "blurred_mongoose",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            2,
            1,
        ),
        (
            "scragnoth",
            ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            3,
            4,
        ),
        (
            "terra_stomper",
            ManaGift {
                g: 3,
                c: 3,
                ..Default::default()
            },
            8,
            8,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(64_031_000 + index as u64);
        let creature = cast_named(&mut e, 0, card, gift(&mana), vec![]);
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        let counterspell = cast_named(
            &mut e,
            1,
            "counterspell",
            ManaGift {
                u: 2,
                ..Default::default()
            },
            target_object(creature),
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{card}: the creature spell is not countered"
        );
        assert_eq!(
            e.state.objects[&counterspell].zone,
            Zone::Graveyard,
            "{card}: the counterspell still goes to the graveyard"
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(power), Some(toughness)),
            "{card}: printed body"
        );
    }
}

#[test]
fn q64_uncounterable_keywords_match_their_printed_cards() {
    let mut e = main1_engine(64_032_001);
    let mongoose = inject_creature_on_battlefield(&mut e, 0, "blurred_mongoose");
    let terra = inject_creature_on_battlefield(&mut e, 0, "terra_stomper");
    assert!(e.effective_has_keyword(mongoose, Keyword::Shroud));
    assert!(e.effective_has_keyword(terra, Keyword::Trample));
    assert!(!e.effective_has_keyword(terra, Keyword::Shroud));
}
