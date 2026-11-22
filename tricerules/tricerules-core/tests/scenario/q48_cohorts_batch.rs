//! Actual-card scenarios for the queue-v48 reuse-only cohorts.
//!
//! Expectations are printed Oracle values. CR 604/611/613 govern the static anthems;
//! CR 701.22/121.2 govern scry-then-draw; CR 115.1/701.8/702.29 govern targeted
//! destruction plus cycling; CR 611.2/613.4/514.2 govern pumps; CR 701.6 governs
//! counterspells; CR 603.6a/701.26/122.1f govern entering taps and stun counters;
//! CR 605/106/118 govern mana abilities; CR 701.8/105.2 govern filtered mass
//! destruction; CR 702.19/702.21/702.29 govern the landcycling creatures.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, ActivateAbility, ChooseTriggerTarget, RuledCommand,
};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
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
) {
    inject_card_into_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    e.apply_scenario_command(player, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
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

fn choose_target(oid: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(oid),
        })),
    }
}

fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect();
    e.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

// ── 01 team anthem ─────────────────────────────────────────────────────────

#[test]
fn q48_team_anthem_canary_simplest_scopes_white_creatures() {
    let mut e = main1_engine(
        48_010_001,
        &["honor_of_the_pure", "savannah_lions"],
        &["savannah_lions"],
    );
    move_ready_to_battlefield(&mut e, 0, "honor_of_the_pure");
    let own_white = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
    let own_green = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing_white = move_ready_to_battlefield(&mut e, 1, "savannah_lions");
    assert_eq!(e.effective_power(own_white), Some(3));
    assert_eq!(e.effective_toughness(own_white), Some(2));
    assert_eq!(e.effective_power(own_green), Some(2), "green is unaffected");
    assert_eq!(
        e.effective_power(opposing_white),
        Some(2),
        "opponent's white creature is unaffected"
    );
    cast_named(
        &mut e,
        0,
        "back_to_nature",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(own_white),
        Some(2),
        "the anthem ends when the source leaves"
    );
}

#[test]
fn q48_team_anthem_canary_hardest_scopes_other_flyers_only() {
    let mut e = main1_engine(
        48_010_002,
        &["spirit_of_the_spires", "cloud_spirit"],
        &["cloud_spirit"],
    );
    let spirit = move_ready_to_battlefield(&mut e, 0, "spirit_of_the_spires");
    let own_flier = move_ready_to_battlefield(&mut e, 0, "cloud_spirit");
    let own_ground = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing_flier = move_ready_to_battlefield(&mut e, 1, "cloud_spirit");
    assert_eq!(
        e.effective_toughness(own_flier),
        Some(2),
        "other flier +0/+1"
    );
    assert_eq!(
        e.effective_toughness(spirit),
        Some(4),
        "the source is not another creature"
    );
    assert_eq!(
        e.effective_toughness(own_ground),
        Some(2),
        "nonflier unaffected"
    );
    assert_eq!(
        e.effective_toughness(opposing_flier),
        Some(1),
        "opponent's flier unaffected"
    );
    // Lethal marked damage becomes lethal if the Spirit leaves and the toughness bonus ends.
    cast_named(
        &mut e,
        0,
        "go_for_the_throat",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        target_object(spirit),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_toughness(own_flier), Some(1));
}

#[test]
fn q48_team_anthem_cards_apply_their_printed_scope() {
    struct Case {
        id: &'static str,
        support: &'static str,
        other: &'static str,
        expected_self: (u32, u32),
        expected_other: (u32, u32),
        expected_nonmatch: (u32, u32),
    }
    for (index, case) in [
        Case {
            id: "honor_of_the_pure",
            support: "savannah_lions",
            other: "grizzly_bears",
            expected_self: (0, 0),
            expected_other: (3, 2),
            expected_nonmatch: (2, 2),
        },
        Case {
            id: "jacques_le_vert",
            support: "grizzly_bears",
            other: "savannah_lions",
            expected_self: (3, 4),
            expected_other: (2, 4),
            expected_nonmatch: (2, 1),
        },
        Case {
            id: "kaysa",
            support: "grizzly_bears",
            other: "savannah_lions",
            expected_self: (3, 4),
            expected_other: (3, 3),
            expected_nonmatch: (2, 1),
        },
        Case {
            id: "meng_huo,_barbarian_king",
            support: "grizzly_bears",
            other: "savannah_lions",
            expected_self: (4, 4),
            expected_other: (3, 3),
            expected_nonmatch: (2, 1),
        },
        Case {
            id: "windstorm_drake",
            support: "cloud_spirit",
            other: "grizzly_bears",
            expected_self: (3, 3),
            expected_other: (4, 1),
            expected_nonmatch: (2, 2),
        },
        Case {
            id: "spirit_of_the_spires",
            support: "cloud_spirit",
            other: "grizzly_bears",
            expected_self: (2, 4),
            expected_other: (3, 2),
            expected_nonmatch: (2, 2),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let own: Vec<&str> = vec![case.id, case.support, case.other];
        let mut e = main1_engine(48_011_000 + index as u64, &own, &[]);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let support = move_ready_to_battlefield(&mut e, 0, case.support);
        let nonmatch = move_ready_to_battlefield(&mut e, 0, case.other);
        if case.expected_self == (0, 0) {
            // Honor of the Pure has no body; its printed characteristics are asserted by the
            // registry test.
        } else {
            assert_eq!(
                e.effective_power(source),
                Some(case.expected_self.0),
                "{}",
                case.id
            );
            assert_eq!(
                e.effective_toughness(source),
                Some(case.expected_self.1),
                "{}",
                case.id
            );
        }
        if case.expected_other == (0, 0) {
            continue;
        }
        assert_eq!(
            e.effective_power(support),
            Some(case.expected_other.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(support),
            Some(case.expected_other.1),
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(nonmatch), e.effective_toughness(nonmatch)),
            (
                Some(case.expected_nonmatch.0),
                Some(case.expected_nonmatch.1)
            ),
            "{}",
            case.id
        );
    }
}

// ── 02 scry then draw ──────────────────────────────────────────────────────

#[test]
fn q48_scry_draw_canary_simplest_bottoms_both_then_draws() {
    let mut e = main1_engine(48_020_001, &["deliberate"], &[]);
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "cloud_spirit", "island"]);
    let hand_before = e.state.players[0].hand.len();
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "deliberate",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let choice = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!(choice.candidate_object_ids, vec![top[0], top[1]]);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[1], top[0]]))
        .expect("bottom both");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "cast one, draw one"
    );
    assert!(e.state.players[0].hand.contains(&top[2]));
}

#[test]
fn q48_scry_draw_canary_hardest_keeps_four_then_draws_two() {
    let mut e = main1_engine(48_020_002, &["foresee"], &[]);
    let top = seat_on_top(
        &mut e,
        0,
        &[
            "grizzly_bears",
            "cloud_spirit",
            "island",
            "savannah_lions",
            "mountain",
        ],
    );
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "foresee",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let hand_after_cast = e.state.players[0].hand.len();
    let choice = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!(choice.candidate_object_ids, top[..4].to_vec());
    e.apply_scenario_command(
        0,
        &submit_resolution_choice(vec![top[3], top[2], top[1], top[0]]),
    )
    .expect("bottom all four");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_after_cast + 2,
        "draw two"
    );
    assert!(e.state.players[0].hand.contains(&top[4]));
}

#[test]
fn q48_scry_draw_cards_draw_their_printed_count() {
    for (index, (id, mana, scry, draw)) in [
        (
            "deliberate",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            2,
            1,
        ),
        (
            "foresee",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            4,
            2,
        ),
        (
            "introduction_to_prophecy",
            ManaGift {
                c: 3,
                ..Default::default()
            },
            2,
            1,
        ),
        (
            "tamiyos_epiphany",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            4,
            2,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(48_021_000 + index as u64, &[id], &[]);
        let cards: Vec<&str> = [
            "grizzly_bears",
            "cloud_spirit",
            "island",
            "savannah_lions",
            "mountain",
            "storm_crow",
        ][..scry + draw]
            .to_vec();
        let top = seat_on_top(&mut e, 0, &cards);
        let batch = cast_instant_and_resolve(&mut e, 0, id, mana);
        let hand_after_cast = e.state.players[0].hand.len();
        let choice = find_resolution_choice(&batch).expect("scry parks");
        assert_eq!(choice.candidate_object_ids.len(), scry, "{id}");
        e.apply_scenario_command(0, &submit_resolution_choice(top[..scry].to_vec()))
            .expect("bottom all scried cards");
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_after_cast + draw,
            "{id}: draw count"
        );
        assert!(
            e.state.players[0].hand.contains(&top[scry]),
            "{id}: first drawn"
        );
        if draw == 2 {
            assert!(
                e.state.players[0].hand.contains(&top[scry + 1]),
                "{id}: second drawn"
            );
        }
    }
}

// ── 03 destroy plus cycling ────────────────────────────────────────────────

fn cycle_from_hand(e: &mut GameEngine, card: &str, cost: ManaGift) {
    let source = inject_card_into_hand(e, 0, card);
    let hand_before = e.state.players[0].hand.len();
    give_mana(e, 0, cost);
    e.apply_scenario_command(0, &hand_ability(e, source, 0))
        .unwrap_or_else(|error| panic!("cycle {card}: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "{card} discards itself"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 1,
        "{card}: cycling discards one and draws one"
    );
}

#[test]
fn q48_destroy_cycling_canary_simplest_destroys_an_artifact() {
    let mut e = main1_engine(48_030_001, &["dissenters_deliverance"], &["bonesplitter"]);
    let artifact = move_ready_to_battlefield(&mut e, 1, "bonesplitter");
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "dissenters_deliverance",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        target_object(artifact),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    cycle_from_hand(
        &mut e,
        "dissenters_deliverance",
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
}

#[test]
fn q48_destroy_cycling_canary_hardest_hits_an_enchantment_and_cycles() {
    let mut e = main1_engine(48_030_002, &["break_asunder"], &["pacifism"]);
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "pacifism");
    cast_named(
        &mut e,
        0,
        "break_asunder",
        ManaGift {
            g: 2,
            c: 2,
            ..Default::default()
        },
        target_object(enchantment),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard);
    cycle_from_hand(
        &mut e,
        "break_asunder",
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
}

#[test]
fn q48_destroy_cycling_cards_apply_their_printed_target_and_cycle() {
    for (index, (id, mana, cycle, either)) in [
        (
            "break_asunder",
            ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
            ManaGift {
                c: 2,
                ..Default::default()
            },
            true,
        ),
        (
            "dissenters_deliverance",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            ManaGift {
                g: 1,
                ..Default::default()
            },
            false,
        ),
        (
            "wilt",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            ManaGift {
                c: 2,
                ..Default::default()
            },
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(
            48_031_000 + index as u64,
            &[id],
            &["bonesplitter", "pacifism", "grizzly_bears"],
        );
        let artifact = move_ready_to_battlefield(&mut e, 1, "bonesplitter");
        let enchantment = inject_permanent_on_battlefield(&mut e, 1, "pacifism");
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let target = if either { enchantment } else { artifact };
        cast_named(&mut e, 0, id, mana, target_object(target));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard, "{id}");
        assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield, "{id}");
        // A creature is never a legal target for these artifact/enchantment spells.
        let hand = inject_card_into_hand(&mut e, 0, id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{id}: creature is not a legal target"
        );
        let _ = hand;
        cycle_from_hand(&mut e, id, cycle);
    }
}

// ── 04 pump plus keyword ───────────────────────────────────────────────────

#[test]
fn q48_pump_keyword_canary_simplest_pumps_and_grants_deathtouch() {
    let mut e = main1_engine(48_040_001, &["lash_of_thorns"], &["grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "lash_of_thorns",
        ManaGift {
            b: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(4));
    assert_eq!(e.effective_toughness(target), Some(3));
    assert!(e.effective_has_keyword(target, Keyword::Deathtouch));
}

#[test]
fn q48_pump_keyword_canary_hardest_grants_flying_and_expires() {
    let mut e = main1_engine(48_040_002, &["mighty_leap"], &["grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "mighty_leap",
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(4));
    assert_eq!(e.effective_toughness(target), Some(4));
    assert!(e.effective_has_keyword(target, Keyword::Flying));
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(target), Some(2), "CR 514.2");
    assert_eq!(e.effective_toughness(target), Some(2));
    assert!(!e.effective_has_keyword(target, Keyword::Flying));
}

#[test]
fn q48_pump_keyword_cards_apply_their_printed_numbers() {
    for (index, (id, mana, power, toughness, keyword)) in [
        (
            "lash_of_thorns",
            ManaGift {
                b: 1,
                ..Default::default()
            },
            2,
            1,
            Keyword::Deathtouch,
        ),
        (
            "mighty_leap",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            2,
            2,
            Keyword::Flying,
        ),
        (
            "screaming_fury",
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            5,
            0,
            Keyword::Haste,
        ),
        (
            "uncanny_speed",
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            3,
            0,
            Keyword::Haste,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(48_041_000 + index as u64, &[id], &["grizzly_bears"]);
        let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        cast_named(&mut e, 0, id, mana, target_object(target));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(target), Some(2 + power), "{id}");
        assert_eq!(e.effective_toughness(target), Some(2 + toughness), "{id}");
        assert!(e.effective_has_keyword(target, keyword), "{id}");
        assert_eq!(
            e.effective_power(bystander),
            Some(2),
            "{id}: only the target"
        );
        assert!(!e.effective_has_keyword(bystander, keyword), "{id}");
    }
}

// ── 05 typed counters ──────────────────────────────────────────────────────

struct CounterCase {
    id: &'static str,
    mana: ManaGift,
    legal: &'static str,
    legal_mana: ManaGift,
    wrong: &'static str,
    wrong_mana: ManaGift,
    hybrid_green: bool,
}

fn counter_cases() -> [CounterCase; 4] {
    [
        CounterCase {
            id: "artifact_blast",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            legal: "bonesplitter",
            legal_mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            wrong: "grizzly_bears",
            wrong_mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            hybrid_green: false,
        },
        CounterCase {
            id: "gainsay",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            legal: "opt",
            legal_mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            wrong: "lightning_bolt",
            wrong_mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            hybrid_green: false,
        },
        CounterCase {
            id: "guttural_response",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            legal: "opt",
            legal_mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            wrong: "divination",
            wrong_mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            hybrid_green: true,
        },
        CounterCase {
            id: "mystic_denial",
            mana: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
            legal: "grizzly_bears",
            legal_mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            wrong: "lightning_bolt",
            wrong_mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            hybrid_green: false,
        },
    ]
}

fn cast_support_spell(e: &mut GameEngine, card: &str, mana: ManaGift) -> u32 {
    ensure_in_hand(e, 0, card);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card);
    let targets = match card {
        "lightning_bolt" => target_player(1),
        "opt" => vec![],
        _ => vec![],
    };
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    e.state.stack.last().expect("spell on stack").id
}

fn cast_counter(e: &mut GameEngine, case: &CounterCase, target: u32) -> Result<(), String> {
    ensure_in_hand(e, 1, case.id);
    give_mana(e, 1, gift(&case.mana));
    let slot = hand_index_for_card(e, 1, case.id);
    let command = cast_spell(slot, target_object(target));
    let _ = case.hybrid_green;
    e.apply_scenario_command(1, &command)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn q48_typed_counters_canary_simplest_counters_an_artifact_spell() {
    let mut e = main1_engine(48_050_001, &["bonesplitter"], &["artifact_blast"]);
    let spell = cast_support_spell(
        &mut e,
        "bonesplitter",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    let case = &counter_cases()[0];
    cast_counter(&mut e, case, spell).expect("counter the artifact spell");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert!(e.state.players[1]
        .graveyard
        .iter()
        .any(|oid| { e.state.objects[oid].card_id == "artifact_blast" }));
}

#[test]
fn q48_typed_counters_canary_hardest_requires_blue_instant_for_hybrid() {
    let mut e = main1_engine(
        48_050_002,
        &["opt", "divination"],
        &["guttural_response", "guttural_response"],
    );
    let instant = cast_support_spell(
        &mut e,
        "opt",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    let case = &counter_cases()[2];
    cast_counter(&mut e, case, instant).expect("counter the blue instant");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&instant].zone, Zone::Graveyard);
    // A blue sorcery is not a blue instant.
    let sorcery = cast_support_spell(
        &mut e,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    assert!(
        cast_counter(&mut e, case, sorcery).is_err(),
        "a blue sorcery is not a legal target"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&sorcery].zone,
        Zone::Graveyard,
        "Divination resolved"
    );
}

#[test]
fn q48_typed_counters_cards_counter_their_printed_filter() {
    for (index, case) in counter_cases().into_iter().enumerate() {
        let mut e = main1_engine(
            48_051_000 + index as u64,
            &[case.legal, case.wrong, "lightning_bolt"],
            &[case.id, case.id],
        );
        let legal = cast_support_spell(&mut e, case.legal, gift(&case.legal_mana));
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        cast_counter(&mut e, &case, legal).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&legal].zone,
            Zone::Graveyard,
            "{}: legal spell countered",
            case.id
        );
        let wrong = cast_support_spell(&mut e, case.wrong, gift(&case.wrong_mana));
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        assert!(
            cast_counter(&mut e, &case, wrong).is_err(),
            "{}: nonmatching spell rejected",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        let expected_zone = if case.wrong == "grizzly_bears" {
            Zone::Battlefield
        } else {
            Zone::Graveyard
        };
        assert_eq!(
            e.state.objects[&wrong].zone, expected_zone,
            "{}: wrong spell resolved normally",
            case.id
        );
    }
}

// ── 06 entering tap ────────────────────────────────────────────────────────

fn resolve_etb_target(e: &mut GameEngine, target: u32) {
    e.apply_scenario_command(0, &choose_target(target))
        .unwrap_or_else(|error| panic!("choose ETB target: {error:?}"));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q48_etb_tap_canary_simplest_taps_an_opponents_creature() {
    let mut e = main1_engine(48_060_001, &["heavy_infantry"], &["grizzly_bears"]);
    let enemy = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "heavy_infantry");
    resolve_etb_target(&mut e, enemy);
    assert!(e.state.objects[&enemy].tapped);
    assert_eq!(e.state.objects[&enemy].counter_count(CounterKind::Stun), 0);
}

#[test]
fn q48_etb_tap_canary_hardest_adds_a_stun_counter() {
    let mut e = main1_engine(48_060_002, &["shipwreck_patrol"], &["grizzly_bears"]);
    let enemy = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "shipwreck_patrol");
    resolve_etb_target(&mut e, enemy);
    assert!(e.state.objects[&enemy].tapped);
    assert_eq!(e.state.objects[&enemy].counter_count(CounterKind::Stun), 1);
    // The stun counter's untap replacement is exercised by the shared engine tests.
}

#[test]
fn q48_etb_tap_cards_apply_their_printed_tap_and_stun() {
    for (index, (id, stun, keyword)) in [
        ("alchemax_slayer-bots", true, None),
        ("chrome_prowler", false, Some(Keyword::Flash)),
        ("fractal_mascot", true, Some(Keyword::Trample)),
        ("heavy_infantry", false, None),
        ("shipwreck_patrol", true, None),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(
            48_061_000 + index as u64,
            &[id],
            &["grizzly_bears", "savannah_lions"],
        );
        let enemy = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let other = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
        let source = move_ready_to_battlefield(&mut e, 0, id);
        if let Some(keyword) = keyword {
            assert!(e.effective_has_keyword(source, keyword), "{id}");
        }
        resolve_etb_target(&mut e, enemy);
        assert!(e.state.objects[&enemy].tapped, "{id}");
        assert_eq!(
            e.state.objects[&enemy].counter_count(CounterKind::Stun),
            u32::from(stun),
            "{id}"
        );
        assert!(
            !e.state.objects[&other].tapped,
            "{id}: only the chosen target"
        );
    }
}

// ── 07 any-color mana ──────────────────────────────────────────────────────

struct ManaCase {
    id: &'static str,
    ability_index: u32,
    cost: ManaGift,
    taps: bool,
    is_creature: bool,
    etb_life: bool,
}

fn mana_cases() -> [ManaCase; 6] {
    [
        ManaCase {
            id: "centaur_nurturer",
            ability_index: 0,
            cost: ManaGift::default(),
            taps: true,
            is_creature: true,
            etb_life: true,
        },
        ManaCase {
            id: "dawnhart_rejuvenator",
            ability_index: 0,
            cost: ManaGift::default(),
            taps: true,
            is_creature: true,
            etb_life: true,
        },
        ManaCase {
            id: "henge_of_ramos",
            ability_index: 1,
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            taps: true,
            is_creature: false,
            etb_life: false,
        },
        ManaCase {
            id: "orochi_leafcaller",
            ability_index: 0,
            cost: ManaGift {
                g: 1,
                ..Default::default()
            },
            taps: false,
            is_creature: true,
            etb_life: false,
        },
        ManaCase {
            id: "prismite",
            ability_index: 0,
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            taps: false,
            is_creature: true,
            etb_life: false,
        },
        ManaCase {
            id: "school_of_the_unseen",
            ability_index: 1,
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            taps: true,
            is_creature: false,
            etb_life: false,
        },
    ]
}

#[test]
fn q48_any_color_mana_canary_simplest_produces_the_chosen_color() {
    let mut e = main1_engine(48_070_001, &["orochi_leafcaller"], &[]);
    let source = move_ready_to_battlefield(&mut e, 0, "orochi_leafcaller");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let mut command = activate_ability_for(&e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 1;
    e.apply_scenario_command(0, &command)
        .expect("activate for blue");
    assert_eq!(pool(&e), (0, 1, 0, 0, 0, 0));
    assert!(!e.state.objects[&source].tapped, "no tap cost");
}

#[test]
fn q48_any_color_mana_canary_hardest_gains_life_then_taps_for_a_color() {
    let mut e = main1_engine(48_070_002, &["centaur_nurturer"], &[]);
    move_ready_to_battlefield(&mut e, 0, "centaur_nurturer");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 23, "enters trigger gains 3");
    let source = battlefield_object_for_card(&e, 0, "centaur_nurturer");
    let mut command = activate_ability_for(&e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 4;
    e.apply_scenario_command(0, &command)
        .expect("activate for green");
    assert_eq!(pool(&e), (0, 0, 0, 0, 1, 0));
    assert!(e.state.objects[&source].tapped);
}

#[test]
fn q48_any_color_mana_cards_offer_all_five_colors() {
    for (index, case) in mana_cases().into_iter().enumerate() {
        for option in 0u32..5 {
            let mut e = main1_engine(
                48_071_000 + index as u64 * 10 + u64::from(option),
                &[case.id],
                &[],
            );
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            if case.is_creature && case.etb_life {
                // Direct fixture insertion skips the enters trigger; the canary covers it.
            }
            give_mana(&mut e, 0, ManaGift { ..case.cost });
            let mut command = activate_ability_for(&e, source, case.ability_index, vec![]);
            let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
                unreachable!()
            };
            ability.mana_option_index = option;
            e.apply_scenario_command(0, &command)
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            let mut expected = [0u32; 6];
            expected[option as usize] = 1;
            assert_eq!(
                pool(&e),
                (
                    expected[0],
                    expected[1],
                    expected[2],
                    expected[3],
                    expected[4],
                    expected[5]
                ),
                "{} option {option}",
                case.id
            );
            assert_eq!(e.state.objects[&source].tapped, case.taps, "{}", case.id);
            assert!(
                e.state.stack.is_empty(),
                "{}: mana ability does not use the stack",
                case.id
            );
        }
    }
}

#[test]
fn q48_any_color_mana_lands_also_tap_for_colorless() {
    for (index, id) in ["henge_of_ramos", "school_of_the_unseen"]
        .into_iter()
        .enumerate()
    {
        let mut e = main1_engine(48_072_000 + index as u64, &[id], &[]);
        let source = inject_permanent_on_battlefield(&mut e, 0, id);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 1), "{id}: one colorless mana");
        assert!(e.state.objects[&source].tapped, "{id}: tap cost");
        assert!(
            e.state.stack.is_empty(),
            "{id}: mana ability does not use the stack"
        );
    }
}

// ── 08 mass destroy with one filter ────────────────────────────────────────

fn creature_names(card: &str) -> bool {
    matches!(card, "grizzly_bears" | "savannah_lions")
}

#[test]
fn q48_mass_destroy_canary_simplest_hits_all_green_creatures() {
    let mut e = main1_engine(
        48_080_001,
        &["natures_ruin"],
        &["grizzly_bears", "savannah_lions"],
    );
    let own_green = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing_green = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let opposing_white = move_ready_to_battlefield(&mut e, 1, "savannah_lions");
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    cast_named(
        &mut e,
        0,
        "natures_ruin",
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&own_green].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing_green].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing_white].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn q48_mass_destroy_canary_hardest_hits_white_permanents() {
    let mut e = main1_engine(48_080_002, &["anarchy"], &["savannah_lions"]);
    let white_creature = move_ready_to_battlefield(&mut e, 1, "savannah_lions");
    let white_enchantment = inject_permanent_on_battlefield(&mut e, 0, "pacifism");
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    let green = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "anarchy",
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&white_creature].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&white_enchantment].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&green].zone, Zone::Battlefield);
}

#[test]
fn q48_mass_destroy_cards_apply_their_printed_filter() {
    for (index, (id, mana, matching_card, other_card, _subtype)) in [
        (
            "acid_rain",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            "forest",
            "island",
            true,
        ),
        (
            "anarchy",
            ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
            "pacifism",
            "bonesplitter",
            false,
        ),
        (
            "flashfires",
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            "plains",
            "forest",
            true,
        ),
        (
            "natures_ruin",
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            "grizzly_bears",
            "savannah_lions",
            false,
        ),
        (
            "virtues_ruin",
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            "savannah_lions",
            "grizzly_bears",
            false,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(48_081_000 + index as u64, &[id], &[]);
        let creature = |e: &mut GameEngine, card: &str| -> u32 {
            match card {
                "grizzly_bears" => inject_creature_with_stats(e, 1, card, 2, 2),
                "savannah_lions" => inject_creature_with_stats(e, 1, card, 2, 1),
                _ => inject_creature_with_stats(e, 1, card, 2, 2),
            }
        };
        let matching = if creature_names(matching_card) {
            creature(&mut e, matching_card)
        } else {
            inject_permanent_on_battlefield(&mut e, 1, matching_card)
        };
        let other = if creature_names(other_card) {
            creature(&mut e, other_card)
        } else {
            inject_permanent_on_battlefield(&mut e, 1, other_card)
        };
        cast_named(&mut e, 0, id, mana, vec![]);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&matching].zone,
            Zone::Graveyard,
            "{id}: {matching_card}"
        );
        assert_eq!(
            e.state.objects[&other].zone,
            Zone::Battlefield,
            "{id}: {other_card}"
        );
    }
}

// ── 09 landcycling creatures ───────────────────────────────────────────────

fn cycle_engine(seed: u64, _card: &str) -> GameEngine {
    let decks = Some(vec![
        vec!["grizzly_bears".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
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

fn cycle_creature(e: &mut GameEngine, id: &str) {
    let source = inject_card_into_hand(e, 0, id);
    let mountain = inject_library_card(e, 0, "mountain");
    let forest = inject_library_card(e, 0, "forest");
    give_mana(
        e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(e, source, 0))
        .expect("activate basic landcycling");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    e.apply_scenario_command(0, &pass())
        .expect("controller passes");
    let search = e
        .apply_scenario_command(1, &pass())
        .expect("basic landcycling resolves to a private search");
    let choice = find_resolution_choice(&search).expect("private library search");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, vec![mountain, forest]);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose the basic land");
    assert_eq!(e.state.objects[&forest].zone, Zone::Hand);
}

#[test]
fn q48_landcycling_canary_simplest_cycles_for_a_basic_land() {
    let mut e = cycle_engine(48_090_001, "horta");
    inject_card_into_hand(&mut e, 0, "horta");
    let horta = put_creature_on_battlefield(&mut e, 0, "horta");
    assert!(e.effective_has_keyword(horta, Keyword::Trample));
    assert_eq!(
        (e.effective_power(horta), e.effective_toughness(horta)),
        (Some(7), Some(6))
    );
    cycle_creature(&mut e, "horta");
}

#[test]
fn q48_landcycling_canary_hardest_keeps_the_ward_cost() {
    let mut e = main1_engine(48_090_002, &["unsummon"], &["humpback_whales"]);
    let whales = move_ready_to_battlefield(&mut e, 1, "humpback_whales");
    assert!(e.effective_has_keyword(whales, Keyword::Vigilance));
    cast_named(
        &mut e,
        0,
        "unsummon",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        target_object(whales),
    );
    assert_eq!(e.state.stack.len(), 2, "Ward is above Unsummon");
    pass_both_players(&mut e);
    let payment = e
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward payment")
        .continuation
        .mana_payment()
        .expect("mana payment");
    assert_eq!(payment.generic_mana_cost, 2);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline),
    )
    .expect("decline the ward cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&whales].zone,
        Zone::Battlefield,
        "Unsummon is countered"
    );
    assert_eq!(e.state.players[0].graveyard.len(), 1);
    let _ = e.state.players[0].graveyard.len();
}

#[test]
fn q48_landcycling_cards_cycle_for_a_basic_land() {
    for (index, id) in ["horta", "humpback_whales", "shardless_outlander"]
        .into_iter()
        .enumerate()
    {
        let mut e = cycle_engine(48_091_000 + index as u64, id);
        cycle_creature(&mut e, id);
    }
}
