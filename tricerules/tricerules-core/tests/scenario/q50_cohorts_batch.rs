//! Actual-card scenarios for the queue-v50 cohorts (families 01-09).
//!
//! Expectations are printed Oracle values. CR 400.3 and 608.2b govern the
//! placement spells; CR 115.1, 400.7 and 404.1 govern filtered graveyard
//! returns; CR 122.1, 502.3 and 614.5 govern stun counters and untap
//! replacement. Pinned rulings are cited in the family records.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    CastMethod, CastSpell, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

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

fn engine(seed: u64, land: &str, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(land, &[card]),
        deck_with("forest", &["grizzly_bears", "bonesplitter", "oakenform"]),
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

fn cast_at(e: &mut GameEngine, card: &str, mana: &ManaGift, target: u32) -> Result<(), String> {
    ensure_in_hand(e, 0, card);
    give_mana(e, 0, gift(mana));
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

// ── 01 put target on top of its owner's library ────────────────────────────

#[test]
fn q50_put_top_canary_simplest_repel_puts_the_target_on_top() {
    let mut e = engine(50_010_001, "island", "repel");
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_at(
        &mut e,
        "repel",
        &ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target,
    )
    .expect("cast Repel");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact mana");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Library);
    assert_eq!(e.state.objects[&target].owner, 1);
    assert_eq!(e.state.players[1].library.front().copied(), Some(target));
    assert!(!e.state.players[1].battlefield.contains(&target));
}

#[test]
fn q50_put_top_canary_hardest_banishment_decree_accepts_three_card_types() {
    for (index, (card, mana)) in [
        (
            "bonesplitter",
            ManaGift {
                w: 2,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "crusade",
            ManaGift {
                w: 2,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "grizzly_bears",
            ManaGift {
                w: 2,
                c: 3,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(50_011_000 + index as u64, "plains", "banishment_decree");
        let target = inject_permanent_on_battlefield(&mut e, 1, card);
        cast_at(&mut e, "banishment_decree", &mana, target).expect("cast the decree");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Library,
            "{card}: a legal card type is placed on top"
        );
        assert_eq!(e.state.players[1].library.front().copied(), Some(target));
    }
}

#[test]
fn q50_put_top_cards_put_only_their_matching_permanents_on_top() {
    struct Case {
        id: &'static str,
        mana: ManaGift,
        target: &'static str,
        illegal: &'static str,
    }
    for (index, case) in [
        Case {
            id: "repel",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            target: "grizzly_bears",
            illegal: "forest",
        },
        Case {
            id: "grasp_of_phantoms",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            target: "grizzly_bears",
            illegal: "forest",
        },
        Case {
            id: "run_aground",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            target: "bonesplitter",
            illegal: "forest",
        },
        Case {
            id: "banishment_decree",
            mana: ManaGift {
                w: 2,
                c: 3,
                ..Default::default()
            },
            target: "crusade",
            illegal: "forest",
        },
        Case {
            id: "temporal_eddy",
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            target: "grizzly_bears",
            illegal: "bonesplitter",
        },
        Case {
            id: "fallow_earth",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            target: "forest",
            illegal: "grizzly_bears",
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(50_012_000 + index as u64, "island", case.id);
        let target = if case.target == "forest" {
            inject_permanent_on_battlefield(&mut e, 1, "forest")
        } else {
            inject_permanent_on_battlefield(&mut e, 1, case.target)
        };
        cast_at(&mut e, case.id, &case.mana, target).expect("cast the placement spell");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Library,
            "{}: the matching permanent moved",
            case.id
        );
        assert_eq!(e.state.objects[&target].owner, 1, "{}", case.id);
        assert_eq!(
            e.state.players[1].library.front(),
            Some(&target),
            "{}: the permanent is on top of its owner's library",
            case.id
        );

        let mut e = engine(50_013_000 + index as u64, "island", case.id);
        let illegal = inject_permanent_on_battlefield(&mut e, 1, case.illegal);
        assert!(
            cast_at(&mut e, case.id, &case.mana, illegal).is_err(),
            "{}: an illegal card type is rejected before casting",
            case.id
        );
        assert_eq!(
            e.state.objects[&illegal].zone,
            Zone::Battlefield,
            "{}: the illegal permanent stays",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q50_put_top_cards_accept_each_disjunctive_branch() {
    for (index, (id, mana, target_card)) in [
        (
            "run_aground",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            "grizzly_bears",
        ),
        (
            "run_aground",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            "bonesplitter",
        ),
        (
            "banishment_decree",
            ManaGift {
                w: 2,
                c: 3,
                ..Default::default()
            },
            "crusade",
        ),
        (
            "temporal_eddy",
            ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            "forest",
        ),
        (
            "temporal_eddy",
            ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            "grizzly_bears",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(50_013_500 + index as u64, "island", id);
        let target = inject_permanent_on_battlefield(&mut e, 1, target_card);
        cast_at(&mut e, id, &mana, target)
            .unwrap_or_else(|error| panic!("{id} at {target_card}: {error:?}"));
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Library,
            "{id} accepts {target_card}"
        );
        assert_eq!(
            e.state.objects[&target].owner, 1,
            "{id}: the owner's library"
        );
        assert!(
            e.state.players[1].library.front() == Some(&target),
            "{id}: the permanent is on top"
        );
    }
}

#[test]
fn q50_put_top_cards_fizzle_when_the_target_leaves() {
    for (index, (id, mana)) in [
        (
            "repel",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "grasp_of_phantoms",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "run_aground",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "banishment_decree",
            ManaGift {
                w: 2,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "temporal_eddy",
            ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "fallow_earth",
            ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(50_014_000 + index as u64, "island", id);
        let target = if id == "fallow_earth" {
            inject_permanent_on_battlefield(&mut e, 1, "forest")
        } else {
            inject_permanent_on_battlefield(&mut e, 1, "grizzly_bears")
        };
        cast_at(&mut e, id, &mana, target).expect("cast the placement spell");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        e.state.players[1].exile.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{id}");
        assert!(
            !e.state.players[1].library.contains(&target),
            "{id}: a stale target is not placed on the library"
        );
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

fn flashback_cast(spell: u32, target: u32, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets: target_object(target),
            ..Default::default()
        })),
    }
}

#[test]
fn q50_put_top_grasp_of_phantoms_flashes_back_and_exiles() {
    let mut e = engine(50_015_001, "island", "grasp_of_phantoms");
    let first = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_at(
        &mut e,
        "grasp_of_phantoms",
        &ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        first,
    )
    .expect("cast from hand");
    pass_both_players(&mut e);
    assert_eq!(e.state.players[1].library.front().copied(), Some(first));
    let spell = e.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == "grasp_of_phantoms")
        .expect("the sorcery is in the graveyard");
    let generation = e.state.zone_change_generation[&spell];
    let second = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 7,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &flashback_cast(spell, second, generation))
        .expect("cast with flashback");
    assert_eq!(e.state.stack.len(), 1);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[1].library.front().copied(), Some(second));
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "a flashback spell is exiled instead of returning to the graveyard"
    );
}

// ── 02 filtered graveyard cards to hand ────────────────────────────────────

struct GraveyardCase {
    id: &'static str,
    mana: ManaGift,
    matching: &'static str,
    nonmatching: &'static str,
}

fn graveyard_cases() -> [GraveyardCase; 4] {
    [
        GraveyardCase {
            id: "boggart_birth_rite",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            matching: "goblin_piker",
            nonmatching: "grizzly_bears",
        },
        GraveyardCase {
            id: "argivian_find",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            matching: "bonesplitter",
            nonmatching: "grizzly_bears",
        },
        GraveyardCase {
            id: "natures_spiral",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            matching: "bonesplitter",
            nonmatching: "divination",
        },
        GraveyardCase {
            id: "deja_vu",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            matching: "divination",
            nonmatching: "grizzly_bears",
        },
    ]
}

#[test]
fn q50_graveyard_to_hand_canary_simplest_boggart_birth_rite_returns_a_goblin() {
    let case = &graveyard_cases()[0];
    let mut e = engine(50_020_001, "swamp", case.id);
    let card = inject_graveyard_card(&mut e, 0, case.matching);
    cast_at(&mut e, case.id, &case.mana, card).expect("cast Boggart Birth Rite");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&card].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&card));
    assert!(!e.state.players[0].graveyard.contains(&card));
    assert_eq!(e.state.objects[&card].owner, 0);
    assert_eq!(e.state.objects[&card].controller, 0);
}

#[test]
fn q50_graveyard_to_hand_canary_hardest_natures_spiral_returns_a_permanent_card() {
    let case = &graveyard_cases()[2];
    let mut e = engine(50_021_001, "forest", case.id);
    let card = inject_graveyard_card(&mut e, 0, case.matching);
    cast_at(&mut e, case.id, &case.mana, card).expect("cast Nature's Spiral");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&card].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&card));
}

#[test]
fn q50_graveyard_to_hand_cards_return_only_their_filtered_cards() {
    for (index, case) in graveyard_cases().into_iter().enumerate() {
        let mut e = engine(50_022_000 + index as u64, "forest", case.id);
        let matching = inject_graveyard_card(&mut e, 0, case.matching);
        let nonmatching = inject_graveyard_card(&mut e, 0, case.nonmatching);
        cast_at(&mut e, case.id, &case.mana, matching).expect("cast the return spell");
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&matching].zone, Zone::Hand, "{}", case.id);
        assert_eq!(
            e.state.objects[&nonmatching].zone,
            Zone::Graveyard,
            "{}: the nonmatching card stays",
            case.id
        );
        assert!(
            !e.state.players[0].graveyard.contains(&matching),
            "{}",
            case.id
        );
        assert!(e.state.players[0].hand.contains(&matching), "{}", case.id);
    }
}

#[test]
fn q50_graveyard_to_hand_cards_reject_nonmatching_cards() {
    for (index, case) in graveyard_cases().into_iter().enumerate() {
        let mut e = engine(50_022_500 + index as u64, "forest", case.id);
        let nonmatching = inject_graveyard_card(&mut e, 0, case.nonmatching);
        assert!(
            cast_at(&mut e, case.id, &case.mana, nonmatching).is_err(),
            "{}: the nonmatching card is not a legal target",
            case.id
        );
        assert_eq!(
            e.state.objects[&nonmatching].zone,
            Zone::Graveyard,
            "{}: the rejected spell leaves the card alone",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q50_graveyard_to_hand_boggart_returns_a_noncreature_goblin_card() {
    let case = &graveyard_cases()[0];
    let mut e = engine(50_022_700, "swamp", case.id);
    let tarfire = inject_graveyard_card(&mut e, 0, "tarfire");
    cast_at(&mut e, case.id, &case.mana, tarfire).expect("cast Boggart Birth Rite");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&tarfire].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&tarfire));
}

#[test]
fn q50_graveyard_to_hand_cards_reject_opponent_graveyards_and_fizzle_stale_targets() {
    for (index, case) in graveyard_cases().into_iter().enumerate() {
        let mut e = engine(50_023_000 + index as u64, "forest", case.id);
        let theirs = inject_graveyard_card(&mut e, 1, case.matching);
        assert!(
            cast_at(&mut e, case.id, &case.mana, theirs).is_err(),
            "{}: another player's graveyard card is not a legal target",
            case.id
        );
        assert_eq!(
            e.state.objects[&theirs].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );

        let mut e = engine(50_024_000 + index as u64, "forest", case.id);
        let stale = inject_graveyard_card(&mut e, 0, case.matching);
        cast_at(&mut e, case.id, &case.mana, stale).expect("cast the return spell");
        e.state.players[0].graveyard.retain(|oid| *oid != stale);
        e.state.objects.get_mut(&stale).expect("stale card").zone = Zone::Exile;
        e.state.players[0].exile.push(stale);
        *e.state.zone_change_generation.entry(stale).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&stale].zone, Zone::Exile, "{}", case.id);
        assert!(
            !e.state.players[0].hand.contains(&stale),
            "{}: a stale target is not returned",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

// ── 03 tap plus stun counters ──────────────────────────────────────────────

struct StunCase {
    id: &'static str,
    mana: ManaGift,
    counters: u32,
    opponent_only: bool,
    scry: u32,
}

fn stun_cases() -> [StunCase; 4] {
    [
        StunCase {
            id: "cryogenic_stasis",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            counters: 1,
            opponent_only: false,
            scry: 0,
        },
        StunCase {
            id: "impede_momentum",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            counters: 3,
            opponent_only: false,
            scry: 1,
        },
        StunCase {
            id: "tranquilize",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            counters: 3,
            opponent_only: true,
            scry: 0,
        },
        StunCase {
            id: "freeze_in_place",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            counters: 3,
            opponent_only: true,
            scry: 2,
        },
    ]
}

fn stun_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears"]),
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

#[test]
fn q50_tap_stun_canary_simplest_cryogenic_stasis_taps_and_draws() {
    let case = &stun_cases()[0];
    let mut e = stun_engine(50_030_001, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let hand_before = e.state.players[0].hand.len();
    cast_at(&mut e, case.id, &case.mana, target).expect("cast Cryogenic Stasis");
    pass_both_players(&mut e);
    assert!(e.state.objects[&target].tapped);
    assert_eq!(
        e.state.objects[&target].counters.get(&CounterKind::Stun),
        Some(&case.counters)
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the spell left hand and one card was drawn"
    );
}

#[test]
fn q50_tap_stun_canary_hardest_freeze_in_place_scries_two() {
    let case = &stun_cases()[3];
    let mut e = stun_engine(50_031_001, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let top = vec![
        inject_library_card(&mut e, 0, "storm_crow"),
        inject_library_card(&mut e, 0, "grizzly_bears"),
        inject_library_card(&mut e, 0, "forest"),
    ];
    e.state.players[0].library.retain(|oid| !top.contains(oid));
    for oid in top.iter().rev() {
        e.state.players[0].library.push_front(*oid);
    }
    cast_at(&mut e, case.id, &case.mana, target).expect("cast Freeze in Place");
    pass_both_players(&mut e);
    assert!(e.state.objects[&target].tapped);
    assert_eq!(
        e.state.objects[&target].counters.get(&CounterKind::Stun),
        Some(&case.counters)
    );
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the scry choice is offered");
    assert_eq!(pending.presentation.max, case.scry);
    e.apply_scenario_command(0, &submit_resolution_choice(top[..2].to_vec()))
        .expect("complete the scry");
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q50_tap_stun_cards_tap_and_place_their_printed_counters() {
    for (index, case) in stun_cases().into_iter().enumerate() {
        let mut e = stun_engine(50_032_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_at(&mut e, case.id, &case.mana, target).expect("cast the stun spell");
        pass_both_players(&mut e);
        assert!(e.state.objects[&target].tapped, "{}", case.id);
        assert_eq!(
            e.state.objects[&target].counters.get(&CounterKind::Stun),
            Some(&case.counters),
            "{}: printed stun count",
            case.id
        );
        if case.scry > 0 {
            let pending = e.state.pending_resolution.as_ref().expect("scry choice");
            assert_eq!(pending.presentation.max, case.scry, "{}", case.id);
        }
    }
}

#[test]
fn q50_tap_stun_cards_still_place_counters_on_an_already_tapped_creature() {
    for (index, case) in stun_cases().into_iter().enumerate() {
        let mut e = stun_engine(50_033_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&target).expect("target").tapped = true;
        cast_at(&mut e, case.id, &case.mana, target).expect("cast the stun spell");
        pass_both_players(&mut e);
        assert!(e.state.objects[&target].tapped, "{}", case.id);
        assert_eq!(
            e.state.objects[&target].counters.get(&CounterKind::Stun),
            Some(&case.counters),
            "{}: an already-tapped target still gets the counters",
            case.id
        );
    }
}

#[test]
fn q50_tap_stun_cards_replace_untaps_with_counter_removal() {
    for (index, case) in stun_cases().into_iter().enumerate() {
        let mut e = stun_engine(50_034_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_at(&mut e, case.id, &case.mana, target).expect("cast the stun spell");
        pass_both_players(&mut e);
        if case.scry > 0 {
            let bottom = e.state.players[0]
                .library
                .iter()
                .take(case.scry as usize)
                .copied()
                .collect::<Vec<_>>();
            e.apply_scenario_command(0, &submit_resolution_choice(bottom))
                .expect("put the scried cards on the bottom");
            assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        }
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.active_player_id(), 1, "{}", case.id);
        assert!(
            e.state.objects[&target].tapped,
            "{}: the stun counter replaces the untap",
            case.id
        );
        assert_eq!(
            e.state.objects[&target]
                .counters
                .get(&CounterKind::Stun)
                .copied()
                .unwrap_or(0),
            case.counters - 1,
            "{}: exactly one counter is removed",
            case.id
        );
    }
}

#[test]
fn q50_tap_stun_cards_scope_opponent_only_targets_and_reject_noncreatures() {
    for case in stun_cases() {
        let mut e = stun_engine(50_035_000, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        if case.opponent_only {
            assert!(
                cast_at(&mut e, case.id, &case.mana, own).is_err(),
                "{}: a creature you control is not legal",
                case.id
            );
        }
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        assert!(
            cast_at(&mut e, case.id, &case.mana, land).is_err(),
            "{}: a noncreature is not legal",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q50_tap_stun_freeze_in_place_fizzles_without_scrying_when_the_target_leaves() {
    let case = &stun_cases()[3];
    let mut e = stun_engine(50_036_001, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let top = vec![inject_library_card(&mut e, 0, "storm_crow")];
    e.state.players[0].library.retain(|oid| !top.contains(oid));
    e.state.players[0].library.push_front(top[0]);
    cast_at(&mut e, case.id, &case.mana, target).expect("cast Freeze in Place");
    e.state.players[1].battlefield.retain(|oid| *oid != target);
    e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
    e.state.players[1].graveyard.push(target);
    *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert!(
        e.state.pending_resolution.is_none(),
        "the pinned ruling: no scry when the target is illegal"
    );
    assert_eq!(e.state.players[0].library.front().copied(), Some(top[0]));
}

// ── 04-06 enters-triggered targeted modifications ─────────────────────────

fn choose_trigger_target(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets,
            },
        )),
    }
}

fn advance_to_next_upkeep(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..240 {
        if e.state.turn_instance > starting_turn
            && e.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
}

#[test]
fn q50_etb_shrink_canary_simplest_eyeblight_assassin_shrinks() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        50_040_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["eyeblight_assassin"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "eyeblight_assassin");
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the shrink target");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(1));
    assert_eq!(e.effective_toughness(target), Some(1));
}

#[test]
fn q50_etb_shrink_canary_hardest_black_dragon_flying_shrinks_three() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        50_041_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["black_dragon"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let target = inject_creature_with_stats(&mut e, 1, "craw_wurm", 6, 4);
    move_ready_to_battlefield(&mut e, 0, "black_dragon");
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the shrink target");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(3));
    assert_eq!(e.effective_toughness(target), Some(1));
    let source = battlefield_object_for_card(&e, 0, "black_dragon");
    assert!(e.effective_has_keyword(source, Keyword::Flying));
}

#[test]
fn q50_etb_shrink_cards_target_only_opposing_creatures_and_expire() {
    for (index, (id, power, toughness)) in [
        ("eyeblight_assassin", -1i32, -1i32),
        ("black_dragon", -3i32, -3i32),
        ("faerie_duelist", -2i32, 0i32),
        ("brinebarrow_intruder", -2i32, 0i32),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            50_042_000 + index as u64,
            &[0, 1],
            20,
            Some(vec![deck_with("island", &[id]), deck_with("forest", &[])]),
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opposing = inject_creature_with_stats(&mut e, 1, "craw_wurm", 6, 4);
        move_ready_to_battlefield(&mut e, 0, id);
        assert!(
            e.apply_scenario_command(0, &choose_trigger_target(target_object(own)))
                .is_err(),
            "{id}: a creature you control is not a legal target"
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_object(opposing)))
            .unwrap_or_else(|error| panic!("{id}: choose opponent creature: {error:?}"));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(opposing),
            Some((6 + power) as u32),
            "{id}"
        );
        assert_eq!(
            e.effective_toughness(opposing),
            Some((4 + toughness) as u32),
            "{id}"
        );
        assert_eq!(
            e.effective_power(own),
            Some(2),
            "{id}: own creature untouched"
        );
        advance_to_next_upkeep(&mut e);
        assert_eq!(e.effective_power(opposing), Some(6), "{id}: pump expired");
        assert_eq!(
            e.effective_toughness(opposing),
            Some(4),
            "{id}: pump expired"
        );
    }
}

#[test]
fn q50_etb_pump_canary_simplest_haazda_officer_pumps() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        50_050_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("plains", &["haazda_officer", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "haazda_officer");
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the pump target");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(3));
    assert_eq!(e.effective_toughness(target), Some(3));
}

#[test]
fn q50_etb_pump_canary_hardest_rage_scarred_berserker_grants_indestructible() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        50_051_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["rage-scarred_berserker"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "rage-scarred_berserker");
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the pump target");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(3));
    assert_eq!(e.effective_toughness(target), Some(2));
    assert!(e.effective_has_keyword(target, Keyword::Indestructible));
    advance_to_next_upkeep(&mut e);
    assert_eq!(e.effective_power(target), Some(2));
    assert!(!e.effective_has_keyword(target, Keyword::Indestructible));
}

#[test]
fn q50_etb_pump_cards_target_only_your_creatures_and_expire() {
    for (index, (id, pumped_power, pumped_toughness)) in [
        ("haazda_officer", 3, 3),
        ("herald_of_the_fair", 3, 3),
        ("rage-scarred_berserker", 3, 2),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            50_052_000 + index as u64,
            &[0, 1],
            20,
            Some(vec![
                deck_with("plains", &[id, "grizzly_bears"]),
                deck_with("forest", &[]),
            ]),
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let own = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        move_ready_to_battlefield(&mut e, 0, id);
        assert!(
            e.apply_scenario_command(0, &choose_trigger_target(target_object(opposing)))
                .is_err(),
            "{id}: an opponent's creature is not a legal target"
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_object(own)))
            .unwrap_or_else(|error| panic!("{id}: choose own creature: {error:?}"));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(own), Some(pumped_power), "{id}");
        assert_eq!(e.effective_toughness(own), Some(pumped_toughness), "{id}");
        advance_to_next_upkeep(&mut e);
        assert_eq!(e.effective_power(own), Some(2), "{id}: pump expired");
        assert_eq!(e.effective_toughness(own), Some(2), "{id}: pump expired");
    }
}

#[test]
fn q50_etb_counter_canary_simplest_jade_bearer_adds_a_counter() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        50_060_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["jade_bearer"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let target = inject_creature_on_battlefield(&mut e, 0, "merfolk_traders");
    move_ready_to_battlefield(&mut e, 0, "jade_bearer");
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the Merfolk");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );
}

#[test]
fn q50_etb_counter_canary_hardest_stromkirk_mentor_excludes_source() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        50_061_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["stromkirk_mentor"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let other = inject_creature_on_battlefield(&mut e, 0, "vampire_sovereign");
    let source = move_ready_to_battlefield(&mut e, 0, "stromkirk_mentor");
    assert!(
        e.apply_scenario_command(0, &choose_trigger_target(target_object(source)))
            .is_err(),
        "the source is not another Vampire"
    );
    e.apply_scenario_command(0, &choose_trigger_target(target_object(other)))
        .expect("choose another Vampire");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&other]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert_eq!(
        e.state.objects[&source]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        None
    );
}

#[test]
fn q50_etb_counter_cards_require_another_typed_creature_you_control() {
    for (index, (id, legal, illegal_type)) in [
        ("stromkirk_mentor", "vampire_sovereign", "grizzly_bears"),
        ("sanguine_glorifier", "vampire_sovereign", "grizzly_bears"),
        ("jade_bearer", "merfolk_traders", "grizzly_bears"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            50_062_000 + index as u64,
            &[0, 1],
            20,
            Some(vec![deck_with("forest", &[id]), deck_with("forest", &[])]),
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let wrong = inject_creature_on_battlefield(&mut e, 0, illegal_type);
        let opposing = inject_creature_on_battlefield(&mut e, 1, legal);
        let right = inject_creature_on_battlefield(&mut e, 0, legal);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        assert!(
            e.apply_scenario_command(0, &choose_trigger_target(target_object(wrong)))
                .is_err(),
            "{id}: the wrong subtype is not legal"
        );
        assert!(
            e.apply_scenario_command(0, &choose_trigger_target(target_object(opposing)))
                .is_err(),
            "{id}: an opponent's typed creature is not legal"
        );
        assert!(
            e.apply_scenario_command(0, &choose_trigger_target(target_object(source)))
                .is_err(),
            "{id}: the source is not another creature"
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_object(right)))
            .unwrap_or_else(|error| panic!("{id}: choose the typed creature: {error:?}"));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&right]
                .counters
                .get(&CounterKind::PlusOnePlusOne),
            Some(&1),
            "{id}"
        );
        assert_eq!(
            e.state.objects[&wrong]
                .counters
                .get(&CounterKind::PlusOnePlusOne),
            None,
            "{id}"
        );
    }
}

#[test]
fn q50_etb_triggered_cards_fizzle_stale_targets() {
    for (index, (id, target_card)) in [
        ("eyeblight_assassin", "grizzly_bears"),
        ("black_dragon", "grizzly_bears"),
        ("faerie_duelist", "grizzly_bears"),
        ("brinebarrow_intruder", "grizzly_bears"),
        ("haazda_officer", "grizzly_bears"),
        ("herald_of_the_fair", "grizzly_bears"),
        ("rage-scarred_berserker", "grizzly_bears"),
        ("stromkirk_mentor", "vampire_sovereign"),
        ("sanguine_glorifier", "vampire_sovereign"),
        ("jade_bearer", "merfolk_traders"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            50_063_000 + index as u64,
            &[0, 1],
            20,
            Some(vec![deck_with("forest", &[id]), deck_with("forest", &[])]),
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let owner = if matches!(
            id,
            "eyeblight_assassin" | "black_dragon" | "faerie_duelist" | "brinebarrow_intruder"
        ) {
            1
        } else {
            0
        };
        let target = inject_creature_on_battlefield(&mut e, owner, target_card);
        let observer = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        move_ready_to_battlefield(&mut e, 0, id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
            .unwrap_or_else(|error| panic!("{id}: choose target: {error:?}"));
        e.state.players[owner]
            .battlefield
            .retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        e.state.players[owner].graveyard.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard, "{id}");
        if matches!(
            id,
            "eyeblight_assassin"
                | "black_dragon"
                | "faerie_duelist"
                | "brinebarrow_intruder"
                | "haazda_officer"
                | "herald_of_the_fair"
                | "rage-scarred_berserker"
        ) {
            assert_eq!(e.effective_power(observer), Some(2), "{id}: no stale pump");
            assert_eq!(
                e.effective_toughness(observer),
                Some(2),
                "{id}: no stale pump"
            );
        } else {
            assert_eq!(
                e.state.objects[&target]
                    .counters
                    .get(&CounterKind::PlusOnePlusOne),
                None,
                "{id}: no stale counter"
            );
        }
    }
}

// ── 07 optional basic-land or subtype search into hand ────────────────────

fn select_search_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn search_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

fn reach_search_branch(e: &mut GameEngine, card: &str) {
    move_ready_to_battlefield(e, 0, card);
    pass_both_players(e);
    assert!(
        e.state.pending_resolution.is_some(),
        "{card}: the optional search branch is offered"
    );
    e.apply_scenario_command(0, &select_search_branch(0))
        .unwrap_or_else(|error| panic!("{card}: choose to search: {error:?}"));
    assert!(
        e.state.pending_resolution.is_some(),
        "{card}: the private library choice follows the branch"
    );
}

#[test]
fn q50_basic_land_search_canary_simplest_farfinder_finds_a_basic_land() {
    let mut e = search_engine(50_070_001, "farfinder");
    let forest = inject_library_card(&mut e, 0, "forest");
    let _noise = inject_library_card(&mut e, 0, "divination");
    reach_search_branch(&mut e, "farfinder");
    let choice = e.state.pending_resolution.as_ref().expect("library choice");
    assert!(
        choice.presentation.candidates.contains(&forest),
        "the basic land is offered"
    );
    assert!(
        choice
            .presentation
            .candidates
            .iter()
            .all(|oid| e.state.objects[oid].card_id != "divination"),
        "a nonland card is not offered"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose the basic land");
    assert_eq!(e.state.objects[&forest].zone, Zone::Hand);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q50_basic_land_search_canary_hardest_district_guide_finds_a_gate() {
    let mut e = search_engine(50_071_001, "district_guide");
    let gate = inject_library_card(&mut e, 0, "gateway_plaza");
    let forest = inject_library_card(&mut e, 0, "forest");
    reach_search_branch(&mut e, "district_guide");
    let choice = e.state.pending_resolution.as_ref().expect("library choice");
    assert!(choice.presentation.candidates.contains(&gate));
    assert!(choice.presentation.candidates.contains(&forest));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![gate]))
        .expect("choose the Gate");
    assert_eq!(e.state.objects[&gate].zone, Zone::Hand);
}

#[test]
fn q50_basic_land_search_cards_offer_only_their_printed_filters() {
    for (index, (id, gate_allowed)) in [
        ("gatecreeper_vine", true),
        ("district_guide", true),
        ("farfinder", false),
        ("pilgrims_eye", false),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = search_engine(50_072_000 + index as u64, id);
        let gate = inject_library_card(&mut e, 0, "gateway_plaza");
        let basic = inject_library_card(&mut e, 0, "forest");
        let nonbasic = inject_library_card(&mut e, 0, "evolving_wilds");
        let nonland = inject_library_card(&mut e, 0, "divination");
        reach_search_branch(&mut e, id);
        let choice = e.state.pending_resolution.as_ref().expect("library choice");
        assert!(choice.presentation.candidates.contains(&basic), "{id}");
        assert_eq!(
            choice.presentation.candidates.contains(&gate),
            gate_allowed,
            "{id}: Gate eligibility follows the printed filter"
        );
        assert!(
            !choice.presentation.candidates.contains(&nonbasic),
            "{id}: a nonbasic non-Gate land is not offered"
        );
        assert!(
            !choice.presentation.candidates.contains(&nonland),
            "{id}: nonland cards are not offered"
        );
    }
}

#[test]
fn q50_basic_land_search_cards_may_decline_the_search() {
    for (index, id) in [
        "gatecreeper_vine",
        "district_guide",
        "farfinder",
        "pilgrims_eye",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = search_engine(50_073_000 + index as u64, id);
        move_ready_to_battlefield(&mut e, 0, id);
        pass_both_players(&mut e);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        e.apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the search");
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{id}");
        assert_eq!(e.state.players[0].library.len(), library_before, "{id}");
        assert!(e.state.pending_resolution.is_none(), "{id}");
    }
}

// ── 08 Treasure on death or entry ─────────────────────────────────────────

fn bolt(e: &mut GameEngine, caster: usize, victim: u32) {
    ensure_in_hand(e, caster, "lightning_bolt");
    give_mana(
        e,
        caster as i32,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, caster, "lightning_bolt");
    e.apply_scenario_command(caster as i32, &cast_spell(slot, target_object(victim)))
        .expect("bolt the creature");
}

fn treasure_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card, "swords_to_plowshares"]),
        deck_with(
            "mountain",
            &[card, "lightning_bolt", "swords_to_plowshares"],
        ),
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

#[test]
fn q50_treasure_canary_simplest_dire_fleet_hoarder_makes_a_treasure_on_death() {
    let mut e = treasure_engine(50_080_001, "dire_fleet_hoarder");
    let victim = move_ready_to_battlefield(&mut e, 0, "dire_fleet_hoarder");
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    bolt(&mut e, 1, victim);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
}

#[test]
fn q50_treasure_canary_hardest_patron_of_the_arts_makes_one_on_entry_and_one_on_death() {
    let mut e = treasure_engine(50_081_001, "patron_of_the_arts");
    let source = move_ready_to_battlefield(&mut e, 0, "patron_of_the_arts");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    bolt(&mut e, 1, source);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 2);
}

#[test]
fn q50_treasure_jewel_eyed_cobra_makes_a_treasure_on_death() {
    let mut e = treasure_engine(50_081_501, "jewel-eyed_cobra");
    let victim = move_ready_to_battlefield(&mut e, 0, "jewel-eyed_cobra");
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    bolt(&mut e, 1, victim);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
}

#[test]
fn q50_treasure_cards_credit_the_creatures_controller_and_do_not_trigger_on_exile() {
    for (index, id) in [
        "dire_fleet_hoarder",
        "jewel-eyed_cobra",
        "patron_of_the_arts",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = treasure_engine(50_082_000 + index as u64, id);
        let victim = move_ready_to_battlefield(&mut e, 1, id);
        if id == "patron_of_the_arts" {
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(battlefield_token_oids(&e, 1, "treasure").len(), 1, "{id}");
        }
        ensure_in_hand(&mut e, 0, "swords_to_plowshares");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "swords_to_plowshares");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(victim)))
            .expect("exile the creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&victim].zone, Zone::Exile, "{id}");
        assert_eq!(
            battlefield_token_oids(&e, 1, "treasure").len(),
            if id == "patron_of_the_arts" { 1 } else { 0 },
            "{id}: exile is not dying"
        );
    }
}

// ── 09 destroy artifact or enchantment ────────────────────────────────────

#[test]
fn q50_destroy_canary_simplest_naturalize_destroys_an_artifact() {
    let mut e = engine(50_090_001, "forest", "naturalize");
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    cast_at(
        &mut e,
        "naturalize",
        &ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        artifact,
    )
    .expect("cast Naturalize");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert!(e.state.players[1].graveyard.contains(&artifact));
}

#[test]
fn q50_destroy_canary_hardest_return_to_the_earth_destroys_a_flying_creature() {
    let mut e = engine(50_091_001, "forest", "return_to_the_earth");
    let flyer = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    cast_at(
        &mut e,
        "return_to_the_earth",
        &ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
        flyer,
    )
    .expect("cast Return to the Earth");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&flyer].zone, Zone::Graveyard);
}

#[test]
fn q50_destroy_cards_accept_only_their_printed_target_types() {
    for (index, (id, mana, legal, illegal)) in [
        (
            "naturalize",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            "bonesplitter",
            "grizzly_bears",
        ),
        (
            "naturalize",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            "crusade",
            "grizzly_bears",
        ),
        (
            "natures_chant",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            "bonesplitter",
            "forest",
        ),
        (
            "natures_chant",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            "crusade",
            "forest",
        ),
        (
            "return_to_the_earth",
            ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            "storm_crow",
            "grizzly_bears",
        ),
        (
            "return_to_the_earth",
            ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            "bonesplitter",
            "grizzly_bears",
        ),
        (
            "return_to_the_earth",
            ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            "crusade",
            "forest",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(50_092_000 + index as u64, "forest", id);
        let target = inject_permanent_on_battlefield(&mut e, 1, legal);
        cast_at(&mut e, id, &mana, target).expect("cast at the legal target");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{id}: {legal}"
        );

        let mut e = engine(50_093_000 + index as u64, "forest", id);
        let rejected = inject_permanent_on_battlefield(&mut e, 1, illegal);
        assert!(
            cast_at(&mut e, id, &mana, rejected).is_err(),
            "{id}: {illegal} is not a legal target"
        );
        assert_eq!(e.state.objects[&rejected].zone, Zone::Battlefield, "{id}");
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

#[test]
fn q50_destroy_cards_fizzle_when_the_target_already_left() {
    for (index, (id, mana, target_card)) in [
        (
            "naturalize",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            "bonesplitter",
        ),
        (
            "natures_chant",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            "bonesplitter",
        ),
        (
            "return_to_the_earth",
            ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            "storm_crow",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(50_094_000 + index as u64, "forest", id);
        let target = inject_permanent_on_battlefield(&mut e, 1, target_card);
        cast_at(&mut e, id, &mana, target).expect("cast the destroy spell");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard, "{id}");
        assert!(!e.state.players[0].graveyard.contains(&target), "{id}");
        assert!(e.state.stack.is_empty(), "{id}");
    }
}
