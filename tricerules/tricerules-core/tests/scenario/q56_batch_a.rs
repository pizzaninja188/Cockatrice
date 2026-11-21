//! Actual-card scenarios for the queue-v56 batch A cohorts.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose targets as the spell is
//! cast; CR 611.2a/514.2 end keyword and pump effects at cleanup; CR 701.26 untaps;
//! CR 701.8 destroys; CR 119.3/119.5 apply life loss and gain; CR 608.2b fizzles a spell
//! whose only target is illegal.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct KeywordCase {
    id: &'static str,
    mana: ManaGift,
    keywords: &'static [Keyword],
}

fn keyword_cases() -> [KeywordCase; 7] {
    [
        KeywordCase {
            id: "accelerate",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Haste],
        },
        KeywordCase {
            id: "charge_through",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Trample],
        },
        KeywordCase {
            id: "cloak_of_feathers",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Flying],
        },
        KeywordCase {
            id: "expedite",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Haste],
        },
        KeywordCase {
            id: "lace_with_moonglove",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            keywords: &[Keyword::Deathtouch],
        },
        KeywordCase {
            id: "leap",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Flying],
        },
        KeywordCase {
            id: "poison_the_blade",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Deathtouch],
        },
    ]
}

struct PumpCase {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
}

fn pump_cases() -> [PumpCase; 3] {
    [
        PumpCase {
            id: "refuse_to_yield",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            pump: (2, 7),
        },
        PumpCase {
            id: "savage_surge",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            pump: (2, 2),
        },
        PumpCase {
            id: "veterans_reflexes",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            pump: (1, 1),
        },
    ]
}

struct LandCase {
    id: &'static str,
    mana: ManaGift,
    /// Opponent life delta when the land's controller loses life, or caster gain when positive.
    controller_loses: Option<i32>,
    caster_gains: i32,
}

fn land_cases() -> [LandCase; 4] {
    [
        LandCase {
            id: "despoil",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            controller_loses: Some(2),
            caster_gains: 0,
        },
        LandCase {
            id: "spreading_rot",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            controller_loses: Some(2),
            caster_gains: 0,
        },
        LandCase {
            id: "drain_the_well",
            mana: ManaGift {
                b: 1,
                g: 1,
                c: 2,
                ..Default::default()
            },
            controller_loses: None,
            caster_gains: 2,
        },
        LandCase {
            id: "maw_of_the_mire",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            controller_loses: None,
            caster_gains: 4,
        },
    ]
}

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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("swamp", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

// ---------------------------------------------------------------------------
// 01-keyword-cantrips

fn cast_keyword(e: &mut GameEngine, case: &KeywordCase, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(
        pool(e, 0),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
}

fn check_keywords(e: &GameEngine, object: u32, case: &KeywordCase, expect: bool) {
    for keyword in case.keywords {
        assert_eq!(
            e.effective_has_keyword(object, *keyword),
            expect,
            "{}: {keyword:?}",
            case.id
        );
    }
}

#[test]
fn q56_a_canary_simplest_expedite_grants_haste_and_draws_one() {
    let case = &keyword_cases()[3];
    let mut e = engine(56_100_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let hand_before = e.state.players[0].hand.len();
    cast_keyword(&mut e, case, target);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "no draw before resolution"
    );
    resolve_entire_stack_two_player(&mut e);
    check_keywords(&e, target, case, true);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "exactly one card"
    );
    end_active_turn(&mut e, 0);
    check_keywords(&e, target, case, false);
}

#[test]
fn q56_a_canary_hardest_lace_with_moonglove_grants_deathtouch_and_draws() {
    let case = &keyword_cases()[4];
    let mut e = engine(56_100_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let hand_before = e.state.players[0].hand.len();
    cast_keyword(&mut e, case, target);
    resolve_entire_stack_two_player(&mut e);
    check_keywords(&e, target, case, true);
    assert!(!e.effective_has_keyword(target, Keyword::Haste));
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
}

#[test]
fn q56_a_keyword_cantrips_grant_only_the_printed_keyword_to_the_target() {
    for (index, case) in keyword_cases().iter().enumerate() {
        let mut e = engine(56_101_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        cast_keyword(&mut e, case, target);
        let hand_after_cast = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        resolve_entire_stack_two_player(&mut e);
        check_keywords(&e, target, case, true);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_after_cast + 1,
            "{}: exactly one card is drawn",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: only the caster draws",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: the opponent's library is untouched",
            case.id
        );
        for keyword in [
            Keyword::Haste,
            Keyword::Trample,
            Keyword::Flying,
            Keyword::Deathtouch,
        ] {
            if !case.keywords.contains(&keyword) {
                assert!(
                    !e.effective_has_keyword(target, keyword),
                    "{}: unexpected {keyword:?}",
                    case.id
                );
            }
            assert!(
                !e.effective_has_keyword(bystander, keyword),
                "{}: bystander gained {keyword:?}",
                case.id
            );
        }
    }
}

#[test]
fn q56_a_keyword_cantrips_reject_a_player_target() {
    for (index, case) in keyword_cases().iter().enumerate() {
        let mut e = engine(56_102_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before = pool(&e, 0);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                .is_err(),
            "{}: a player is not a legal creature target",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q56_a_keyword_cantrips_fizzle_without_drawing_when_the_target_leaves() {
    for (index, case) in keyword_cases().iter().enumerate() {
        let mut e = engine(56_103_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        cast_keyword(&mut e, case, target);
        // The cast card leaves the hand, so the resolved hand counts that movement too.
        let after_cast = e.state.players[0].hand.len();
        move_to_graveyard(&mut e, 0, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            after_cast,
            "{}: an illegal target stops the draw",
            case.id
        );
        assert!(
            e.state.players[0]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the spell did not resolve",
            case.id
        );
        assert!(hand_before >= after_cast, "{}", case.id);
    }
}

// ---------------------------------------------------------------------------
// 02-pump-untap

fn cast_pump(e: &mut GameEngine, case: &PumpCase, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

fn check_pump(e: &GameEngine, object: u32, case: &PumpCase, expect: bool) {
    let (power, toughness) = if expect {
        ((2 + case.pump.0) as u32, (2 + case.pump.1) as u32)
    } else {
        (2, 2)
    };
    assert_eq!(e.effective_power(object), Some(power), "{}", case.id);
    assert_eq!(
        e.effective_toughness(object),
        Some(toughness),
        "{}",
        case.id
    );
}

#[test]
fn q56_a_canary_simplest_veterans_reflexes_pumps_and_untaps() {
    let case = &pump_cases()[2];
    let mut e = engine(56_200_001);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.state.objects.get_mut(&target).expect("creature").tapped = true;
    cast_pump(&mut e, case, target);
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.state.objects[&target].tapped);
    check_pump(&e, target, case, true);
    end_active_turn(&mut e, 0);
    check_pump(&e, target, case, false);
}

#[test]
fn q56_a_canary_hardest_refuse_to_yield_applies_the_printed_pump() {
    let case = &pump_cases()[0];
    let mut e = engine(56_200_002);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&target).expect("creature").tapped = true;
    cast_pump(&mut e, case, target);
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.state.objects[&target].tapped);
    check_pump(&e, target, case, true);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(4), Some(9))
    );
}

#[test]
fn q56_a_pump_untap_cards_apply_only_the_printed_values_to_the_target() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = engine(56_201_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        e.state.objects.get_mut(&target).expect("creature").tapped = true;
        cast_pump(&mut e, case, target);
        resolve_entire_stack_two_player(&mut e);
        assert!(!e.state.objects[&target].tapped, "{}", case.id);
        check_pump(&e, target, case, true);
        assert_eq!(e.effective_power(bystander), Some(2), "{}", case.id);
        assert_eq!(e.effective_toughness(bystander), Some(2), "{}", case.id);
    }
}

#[test]
fn q56_a_pump_untap_cards_fizzle_when_the_target_leaves() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = engine(56_202_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_pump(&mut e, case, target);
        move_to_graveyard(&mut e, 0, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            e.state.players[0]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the spell did not resolve",
            case.id
        );
    }
}

#[test]
fn q56_a_pump_untap_cards_reject_a_player_target() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = engine(56_203_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before = pool(&e, 0);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                .is_err(),
            "{}: a player is not a legal creature target",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q56_a_savage_surge_accepts_an_already_untapped_creature() {
    let case = &pump_cases()[1];
    let mut e = engine(56_204_001);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(
        !e.state.objects[&target].tapped,
        "the target starts untapped"
    );
    cast_pump(&mut e, case, target);
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.state.objects[&target].tapped, "it stays untapped");
    check_pump(&e, target, case, true);
}

// ---------------------------------------------------------------------------
// 03-land-destruction-life

fn cast_land(e: &mut GameEngine, case: &LandCase, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

#[test]
fn q56_a_canary_simplest_despoil_destroys_a_land_and_its_controller_loses_two() {
    let case = &land_cases()[0];
    let mut e = engine(56_300_001);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    cast_land(&mut e, case, land);
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert!(e.state.players[1].graveyard.contains(&land));
    assert_eq!(e.state.players[1].life, 18);
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn q56_a_canary_hardest_drain_the_well_pays_hybrid_and_gains_two() {
    let case = &land_cases()[2];
    let mut e = engine(56_300_002);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    cast_land(&mut e, case, land);
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, 22);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q56_a_land_destruction_life_riders_apply_the_printed_values() {
    for (index, case) in land_cases().iter().enumerate() {
        let mut e = engine(56_301_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        cast_land(&mut e, case, land);
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&land].zone, Zone::Graveyard, "{}", case.id);
        assert!(
            e.state.players[1].graveyard.contains(&land),
            "{}: owner's graveyard",
            case.id
        );
        match case.controller_loses {
            Some(amount) => {
                assert_eq!(e.state.players[1].life, 20 - amount, "{}", case.id);
                assert_eq!(e.state.players[0].life, 20, "{}", case.id);
            }
            None => {
                assert_eq!(
                    e.state.players[0].life,
                    20 + case.caster_gains,
                    "{}",
                    case.id
                );
                assert_eq!(e.state.players[1].life, 20, "{}", case.id);
            }
        }
    }
}

#[test]
fn q56_a_land_destruction_cards_reject_nonland_targets() {
    for (index, case) in land_cases().iter().enumerate() {
        let mut e = engine(56_302_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e, 0);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{}: a creature is not a land",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q56_a_land_destruction_cards_do_not_apply_the_rider_to_a_stale_target() {
    for (index, case) in land_cases().iter().enumerate() {
        let mut e = engine(56_303_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        cast_land(&mut e, case, land);
        e.state.players[1].battlefield.retain(|oid| *oid != land);
        e.state.objects.get_mut(&land).expect("land").zone = Zone::Exile;
        e.state.players[1].exile.push(land);
        *e.state.zone_change_generation.entry(land).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&land].zone, Zone::Exile, "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}: no life gained", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}: no life lost", case.id);
    }
}
