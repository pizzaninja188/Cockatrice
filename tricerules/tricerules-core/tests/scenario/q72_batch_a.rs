//! Actual-card scenarios for the queue-v72 batch A cohorts.
//!
//! Expectations are printed Oracle values. Auras attach under CR 303.4a and apply their static
//! layer-7c modifier under CR 613.4c. Cycling is a hand-zone activated ability (CR 702.29a) with
//! its printed cost (CR 118.3). Combat pumps last until cleanup (CR 611.2a/514.2); double strike
//! and first strike apply in layer 6 (CR 613.1f, 702.4, 702.7), indestructible in layer 6
//! (CR 702.12), and investigate creates one Clue artifact token (CR 701.16a, 111.10f). A spell
//! whose only target is illegal does not resolve (CR 608.2b), so its rider does not happen.
use super::helpers::*;

use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, ChoiceKind, RuledCommand,
};

struct AuraCase {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
    keyword: Option<Keyword>,
    cycle: ManaGift,
}

fn aura_cases() -> [AuraCase; 3] {
    [
        AuraCase {
            id: "improvised_armor",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            pump: (2, 5),
            keyword: None,
            cycle: ManaGift {
                c: 3,
                ..Default::default()
            },
        },
        AuraCase {
            id: "savage_hunger",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            pump: (1, 0),
            keyword: Some(Keyword::Trample),
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
        AuraCase {
            id: "sicken",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            pump: (-1, -1),
            keyword: None,
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
    ]
}

struct CombatCase {
    id: &'static str,
    mana: ManaGift,
    cycle: ManaGift,
}

fn combat_cases() -> [CombatCase; 3] {
    [
        CombatCase {
            id: "coordinated_charge",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
        CombatCase {
            id: "pursue_glory",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
        CombatCase {
            id: "raking_claws",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
    ]
}

struct RiderCase {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
    keyword: Keyword,
}

fn rider_cases() -> [RiderCase; 2] {
    [
        RiderCase {
            id: "survive_the_night",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            pump: (1, 0),
            keyword: Keyword::Indestructible,
        },
        RiderCase {
            id: "the_chase_is_on",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            pump: (3, 0),
            keyword: Keyword::FirstStrike,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn combat_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut e);
    e
}

fn finish_turn_from_current_step(e: &mut GameEngine, player: usize) {
    for _ in 0..16 {
        if e.state.active_player_idx != player {
            break;
        }
        if e.apply_scenario_command(player as i32, &primitive_yield())
            .is_err()
        {
            break;
        }
    }
    resolve_cleanup_discards_if_any(e);
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

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn hand_ability(e: &GameEngine, source: u32) -> RuledCommand {
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

fn cycle_from_hand(e: &mut GameEngine, id: &str, cost: ManaGift) {
    let card = inject_card_into_hand(e, 0, id);
    let drawn = put_on_top(e, 0, "grizzly_bears");
    let hand_before = e.state.players[0].hand.len();
    let opponent_hand = e.state.players[1].hand.clone();
    give_mana(e, 0, gift(&cost));
    e.apply_scenario_command(0, &hand_ability(e, card))
        .unwrap_or_else(|error| panic!("{id}: cycle: {error:?}"));
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Graveyard,
        "{id}: cycling discards self"
    );
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{id}: exact cycling cost");
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{id}: draws one");
    assert_eq!(e.state.players[0].hand.len(), hand_before, "{id}");
    assert_eq!(
        e.state.players[1].hand, opponent_hand,
        "{id}: opponent hand unchanged"
    );
}

fn clue_count(e: &GameEngine, player: usize) -> usize {
    battlefield_token_oids(e, player, "clue").len()
}

// ---------------------------------------------------------------------------
// 01-cycling-auras

#[test]
fn q72_cycling_auras_canary_simplest_sicken_applies_minus_one_minus_one_and_cycles() {
    let case = aura_cases()
        .into_iter()
        .find(|case| case.id == "sicken")
        .expect("Sicken");
    let mut e = engine(72_100_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Sicken");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "Sicken: exact mana");
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(1), Some(1)),
        "Sicken: -1/-1"
    );
    assert_eq!(
        (
            e.effective_power(bystander),
            e.effective_toughness(bystander)
        ),
        (Some(2), Some(2)),
        "Sicken: only the attached creature changes"
    );
    let aura = battlefield_object_for_card(&e, 0, case.id);
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(target))
    );
    cycle_from_hand(&mut e, case.id, case.cycle);
}

#[test]
fn q72_cycling_auras_canary_hardest_improvised_armor_applies_plus_two_plus_five_and_cycles() {
    let case = aura_cases()
        .into_iter()
        .find(|case| case.id == "improvised_armor")
        .expect("Improvised Armor");
    let mut e = engine(72_100_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Improvised Armor");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(4), Some(7)),
        "Improvised Armor: +2/+5 on an opponent's creature"
    );
    cycle_from_hand(&mut e, case.id, case.cycle);
}

#[test]
fn q72_cycling_auras_apply_their_printed_static_values_and_cycle() {
    for (index, case) in aura_cases().into_iter().enumerate() {
        let mut e = engine(72_101_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: printed static modifier",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: bystander unchanged",
            case.id
        );
        match case.keyword {
            Some(keyword) => assert!(
                e.effective_has_keyword(target, keyword),
                "{}: printed keyword",
                case.id
            ),
            None => assert!(
                !e.effective_has_keyword(target, Keyword::Trample),
                "{}: no unexpected keyword",
                case.id
            ),
        }
        let aura = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(target)),
            "{}: attached to its target",
            case.id
        );
        cycle_from_hand(&mut e, case.id, case.cycle);
    }
}

#[test]
fn q72_cycling_auras_reject_noncreature_targets_and_unfunded_cycling() {
    for case in aura_cases() {
        let mut e = engine(72_102_000 + case.mana.c as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{}: an Aura cannot enchant a land",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            (
                case.mana.w,
                case.mana.u,
                case.mana.b,
                case.mana.r,
                case.mana.g,
                case.mana.c
            ),
            "{}: rejected cast spends nothing",
            case.id
        );

        let mut cycling = engine(72_102_100 + case.mana.c as u64);
        let card = inject_card_into_hand(&mut cycling, 0, case.id);
        give_mana(
            &mut cycling,
            0,
            ManaGift {
                c: case.cycle.c.saturating_sub(1),
                ..Default::default()
            },
        );
        assert!(
            cycling
                .apply_scenario_command(0, &hand_ability(&cycling, card))
                .is_err(),
            "{}: cycling needs its full printed mana",
            case.id
        );
        assert_eq!(
            cycling.state.objects[&card].zone,
            Zone::Hand,
            "{}: rejected cycling keeps the card in hand",
            case.id
        );
    }
}

// ---------------------------------------------------------------------------
// 02-cycling-combat

fn cast_combat(
    e: &mut GameEngine,
    case: &CombatCase,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact mana", case.id);
    resolve_entire_stack_two_player(e);
}

#[test]
fn q72_cycling_combat_canary_simplest_raking_claws_grants_double_strike_and_cycles() {
    let case = combat_cases()
        .into_iter()
        .find(|case| case.id == "raking_claws")
        .expect("Raking Claws");
    let mut e = engine(72_200_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
    cast_combat(&mut e, &case, target_object(target));
    assert!(
        e.effective_has_keyword(target, Keyword::DoubleStrike),
        "Raking Claws grants double strike"
    );
    assert!(
        !e.effective_has_keyword(bystander, Keyword::DoubleStrike),
        "Raking Claws only grants to its target"
    );
    cycle_from_hand(&mut e, case.id, case.cycle);
    end_active_turn(&mut e, 0);
    assert!(
        !e.effective_has_keyword(target, Keyword::DoubleStrike),
        "Raking Claws grant expires at cleanup"
    );
}

#[test]
fn q72_cycling_combat_canary_hardest_pursue_glory_pumps_all_attackers_and_cycles() {
    let case = combat_cases()
        .into_iter()
        .find(|case| case.id == "pursue_glory")
        .expect("Pursue Glory");
    let mut e = combat_engine(72_200_002);
    let attacker = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
    let nonattacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let opponent = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    cast_combat(&mut e, &case, vec![]);
    assert_eq!(
        (e.effective_power(attacker), e.effective_toughness(attacker)),
        (Some(4), Some(2)),
        "Pursue Glory: attacking creature gets +2/+0"
    );
    assert_eq!(
        (
            e.effective_power(nonattacker),
            e.effective_toughness(nonattacker)
        ),
        (Some(2), Some(2)),
        "Pursue Glory: nonattacking creature unchanged"
    );
    assert_eq!(
        (e.effective_power(opponent), e.effective_toughness(opponent)),
        (Some(2), Some(2)),
        "Pursue Glory: an opponent's nonattacking creature unchanged"
    );
    cycle_from_hand(&mut e, case.id, case.cycle);
}

#[test]
fn q72_cycling_combat_cards_apply_their_printed_scope_and_expire() {
    for (index, case) in combat_cases().into_iter().enumerate() {
        let mut e = if case.id == "pursue_glory" {
            combat_engine(72_201_000 + index as u64)
        } else {
            engine(72_201_000 + index as u64)
        };
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let second = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        if case.id == "raking_claws" {
            cast_combat(&mut e, &case, target_object(first));
            assert!(
                e.effective_has_keyword(first, Keyword::DoubleStrike),
                "Raking Claws target"
            );
            assert!(
                !e.effective_has_keyword(second, Keyword::DoubleStrike),
                "Raking Claws bystander"
            );
            end_active_turn(&mut e, 0);
            assert!(
                !e.effective_has_keyword(first, Keyword::DoubleStrike),
                "Raking Claws expires"
            );
        } else if case.id == "coordinated_charge" {
            cast_combat(&mut e, &case, vec![]);
            for creature in [first, second] {
                assert_eq!(
                    (e.effective_power(creature), e.effective_toughness(creature)),
                    (Some(4), Some(3)),
                    "Coordinated Charge: each controlled creature"
                );
            }
            assert_eq!(
                (e.effective_power(opposing), e.effective_toughness(opposing)),
                (Some(2), Some(2)),
                "Coordinated Charge: opponent unchanged"
            );
            end_active_turn(&mut e, 0);
            assert_eq!(
                (e.effective_power(first), e.effective_toughness(first)),
                (Some(2), Some(2)),
                "Coordinated Charge: expires at cleanup"
            );
        } else {
            e.apply_scenario_command(0, &declare_attackers(vec![first]))
                .expect("declare attacker");
            cast_combat(&mut e, &case, vec![]);
            assert_eq!(
                (e.effective_power(first), e.effective_toughness(first)),
                (Some(4), Some(2)),
                "Pursue Glory: attacker pumped"
            );
            assert_eq!(
                (e.effective_power(second), e.effective_toughness(second)),
                (Some(2), Some(2)),
                "Pursue Glory: nonattacker unchanged"
            );
            finish_turn_from_current_step(&mut e, 0);
            assert_eq!(
                (e.effective_power(first), e.effective_toughness(first)),
                (Some(2), Some(2)),
                "Pursue Glory: expires at cleanup"
            );
        }
    }
}

#[test]
fn q72_cycling_combat_cards_cycle_for_the_printed_cost() {
    for case in combat_cases() {
        let mut e = engine(72_202_000 + case.mana.c as u64);
        cycle_from_hand(&mut e, case.id, case.cycle);
    }
}

#[test]
fn q72_cycling_combat_rejects_wrong_targets_and_unfunded_cycling() {
    let raking = combat_cases()
        .into_iter()
        .find(|case| case.id == "raking_claws")
        .expect("Raking Claws");
    let mut e = engine(72_203_001);
    let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_card_into_hand(&mut e, 0, raking.id);
    give_mana(&mut e, 0, gift(&raking.mana));
    let slot = hand_index_for_card(&e, 0, raking.id);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "Raking Claws requires a creature target"
    );
    let mut cycling = engine(72_203_002);
    let card = inject_card_into_hand(&mut cycling, 0, raking.id);
    give_mana(
        &mut cycling,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    assert!(
        cycling
            .apply_scenario_command(0, &hand_ability(&cycling, card))
            .is_err(),
        "Raking Claws cycling needs its full {{2}}"
    );
}

// ---------------------------------------------------------------------------
// 03-investigate-riders

fn cast_rider(e: &mut GameEngine, case: &RiderCase, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact mana", case.id);
}

#[test]
fn q72_investigate_riders_canary_simplest_the_chase_is_on_pumps_first_strike_and_investigates() {
    let case = rider_cases()
        .into_iter()
        .find(|case| case.id == "the_chase_is_on")
        .expect("The Chase Is On");
    let mut e = engine(72_300_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast_rider(&mut e, &case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(5), Some(2)),
        "The Chase Is On: +3/+0"
    );
    assert!(
        e.effective_has_keyword(target, Keyword::FirstStrike),
        "The Chase Is On: first strike"
    );
    assert_eq!(clue_count(&e, 0), 1, "The Chase Is On: one Clue");
    assert_eq!(clue_count(&e, 1), 0, "The Chase Is On: no opponent Clue");
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2)),
        "The Chase Is On: pump expires"
    );
    assert!(
        !e.effective_has_keyword(target, Keyword::FirstStrike),
        "The Chase Is On: first strike expires"
    );
}

#[test]
fn q72_investigate_riders_canary_hardest_they_went_this_way_searches_tapped_and_investigates() {
    let mut e = engine(72_300_002);
    let basic = inject_library_card(&mut e, 0, "forest");
    e.state.players[0]
        .library
        .retain(|candidate| *candidate != basic);
    e.state.players[0].library.push_front(basic);
    inject_card_into_hand(&mut e, 0, "they_went_this_way");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "they_went_this_way");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast They Went This Way");
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    let search = e
        .apply_scenario_command(1, &pass())
        .expect("resolve search");
    let choice = find_resolution_choice(&search).expect("basic-land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&basic));
    let completion = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![basic]))
        .expect("choose the basic land");
    assert_eq!(e.state.objects[&basic].zone, Zone::Battlefield);
    assert!(e.state.objects[&basic].tapped, "the land enters tapped");
    assert_eq!(clue_count(&e, 0), 1, "They Went This Way: one Clue");
    assert_eq!(clue_count(&e, 1), 0, "They Went This Way: no opponent Clue");
    assert!(
        completion.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )),
        "the search shuffles after choosing"
    );
}

#[test]
fn q72_investigate_riders_apply_their_printed_values_and_create_one_clue() {
    for (index, case) in rider_cases().into_iter().enumerate() {
        let mut e = engine(72_301_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        cast_rider(&mut e, &case, target);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(target, case.keyword),
            "{}: printed keyword",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: bystander unchanged",
            case.id
        );
        assert_eq!(clue_count(&e, 0), 1, "{}: one Clue", case.id);
    }
}

#[test]
fn q72_survive_the_night_indestructible_prevents_destroy() {
    let case = rider_cases()
        .into_iter()
        .find(|case| case.id == "survive_the_night")
        .expect("Survive the Night");
    let mut e = engine(72_302_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast_rider(&mut e, &case, target);
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
    let slot = hand_index_for_card(&e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Battlefield,
        "indestructible survives a destroy effect"
    );
}

#[test]
fn q72_investigate_riders_fizzle_stale_targets_without_clues() {
    for (index, case) in rider_cases().into_iter().enumerate() {
        let mut e = engine(72_303_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast rider");
        move_to_graveyard(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(clue_count(&e, 0), 0, "{}: no Clue on fizzle", case.id);
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: no unrelated life change",
            case.id
        );
    }
}

#[test]
fn q72_investigate_riders_reject_noncreature_targets() {
    let chase = rider_cases()
        .into_iter()
        .find(|case| case.id == "the_chase_is_on")
        .expect("The Chase Is On");
    let mut e = engine(72_304_001);
    let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_card_into_hand(&mut e, 0, chase.id);
    give_mana(&mut e, 0, gift(&chase.mana));
    let slot = hand_index_for_card(&e, 0, chase.id);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "The Chase Is On requires a creature target"
    );
}

#[test]
fn q72_cycling_auras_fizzle_when_the_aura_target_leaves() {
    let mut e = engine(72_103_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    inject_card_into_hand(&mut e, 0, "sicken");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "sicken");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Sicken");
    move_to_graveyard(&mut e, 1, target);
    let spell = e.state.stack.last().expect("Sicken on stack").id;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "the Aura is put into its owner's graveyard when its target is gone"
    );
    assert!(
        battlefield_object_for_card_optional(&e, 0, "sicken").is_none(),
        "the Aura never enters the battlefield"
    );
}

fn battlefield_object_for_card_optional(
    e: &GameEngine,
    player: usize,
    card_id: &str,
) -> Option<u32> {
    e.state.players[player]
        .battlefield
        .iter()
        .copied()
        .find(|oid| {
            e.state
                .objects
                .get(oid)
                .map(|object| object.card_id == card_id)
                .unwrap_or(false)
        })
}

#[test]
fn q72_raking_claws_fizzles_when_the_target_leaves() {
    let mut e = engine(72_204_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    inject_card_into_hand(&mut e, 0, "raking_claws");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "raking_claws");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Raking Claws");
    move_to_graveyard(&mut e, 1, target);
    let spell = e.state.stack.last().expect("Raking Claws on stack").id;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "Raking Claws resolves to the graveyard after its only target is gone"
    );
    assert!(
        !e.effective_has_keyword(target, Keyword::DoubleStrike),
        "no double strike is granted to a departed target"
    );
}

#[test]
fn q72_they_went_this_way_keeps_the_opponent_library_private_and_still_investigates_without_a_basic(
) {
    let mut e = engine(72_305_001);
    let basic = inject_library_card(&mut e, 0, "forest");
    e.state.players[0]
        .library
        .retain(|candidate| *candidate != basic);
    e.state.players[0].library.push_front(basic);
    let opponent_library = e.state.players[1].library.clone();
    inject_card_into_hand(&mut e, 0, "they_went_this_way");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "they_went_this_way");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast They Went This Way");
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    let search = e
        .apply_scenario_command(1, &pass())
        .expect("resolve search");
    let choice = find_resolution_choice(&search).expect("basic-land search choice");
    assert!(choice.candidate_object_ids.contains(&basic));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![basic]))
        .expect("choose the basic land");
    assert_eq!(
        e.state.players[1].library, opponent_library,
        "the opponent's library is not searched or reordered"
    );
    assert_eq!(clue_count(&e, 0), 1, "one Clue");

    let mut empty = engine(72_305_002);
    empty.state.players[0].library.clear();
    inject_card_into_hand(&mut empty, 0, "they_went_this_way");
    give_mana(
        &mut empty,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&empty, 0, "they_went_this_way");
    empty
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast They Went This Way with no basic");
    empty
        .apply_scenario_command(0, &pass())
        .expect("caster pass");
    let search = empty
        .apply_scenario_command(1, &pass())
        .expect("resolve search");
    let choice = find_resolution_choice(&search).expect("empty basic-land search choice");
    assert!(choice.candidate_object_ids.is_empty());
    empty
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("fail to find");
    assert_eq!(
        clue_count(&empty, 0),
        1,
        "failing to find still investigates"
    );
}
