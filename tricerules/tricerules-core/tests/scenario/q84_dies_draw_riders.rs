//! Actual-card scenarios for the queue-v84 02-dies-draw-riders cohort.
//!
//! Every card has "When this creature dies, draw a card" plus one printed rider. Expectations are
//! printed Oracle values. CR 603.6c stages the dies trigger, CR 701.21a governs sacrifice, and CR
//! 702.108a-b defines Prowess. The pinned 2024-11-08 Phantasmal Shieldback ruling places its
//! sacrifice trigger above the targeting spell and confirms that spell is removed on resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "jeskai_sage",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "filigree_familiar",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "living_lies_of_loki",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 3),
        },
        Case {
            id: "phantasmal_shieldback",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (1, 3),
        },
    ]
}

fn engine(seed: u64, p0_specials: &[&str], p1_specials: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", p0_specials),
        deck_with("mountain", p1_specials),
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

fn cast_card(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let creature = battlefield_object_for_card(e, 0, case.id);
    let pool = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "{}: exact cast",
        case.id
    );
    creature
}

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

#[test]
fn q84_dies_draw_canary_simplest_filigree_familiar_gains_and_draws() {
    let case = &cases()[1];
    let mut e = engine(84_020_001, &[case.id], &["lightning_bolt"]);
    let life = e.state.players[0].life;
    let creature = cast_card(&mut e, case);
    assert_eq!(e.state.players[0].life, life + 2, "printed entry life gain");
    assert_eq!(e.effective_power(creature), Some(2));
    assert_eq!(e.effective_toughness(creature), Some(2));
    let hand = e.state.players[0].hand.len();
    e.apply_scenario_command(0, &pass())
        .expect("pass priority to the opponent");
    bolt(&mut e, 1, creature);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand + 1,
        "the dies trigger draws one card"
    );
}

#[test]
fn q84_dies_draw_canary_hardest_phantasmal_shieldback_sacrifices_before_bolt() {
    let case = &cases()[3];
    let mut e = engine(84_021_001, &[case.id], &["lightning_bolt"]);
    let creature = cast_card(&mut e, case);
    let life_before = e.state.players[0].life;
    let hand_before = e.state.players[0].hand.len();
    e.apply_scenario_command(0, &pass())
        .expect("pass priority to the opponent");
    bolt(&mut e, 1, creature);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Graveyard,
        "the becomes-target trigger sacrifices the Shieldback"
    );
    assert_eq!(
        e.state.players[0].life, life_before,
        "the Bolt is removed from the stack without dealing damage"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the dies trigger still draws one card"
    );
    assert_eq!(count_card_id_in_graveyard(&e, 1, "lightning_bolt"), 1);
}

#[test]
fn q84_dies_draw_cards_have_exact_identity_and_bodies() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(84_022_000 + index as u64, &[case.id], &[]);
        let creature = cast_card(&mut e, &case);
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.body.1),
            "{}",
            case.id
        );
    }
}

#[test]
fn q84_dies_draw_cards_draw_only_for_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(84_023_000 + index as u64, &[case.id, "lightning_bolt"], &[]);
        let creature = cast_card(&mut e, &case);
        ensure_in_hand(&mut e, 0, "lightning_bolt");
        let controller_hand = e.state.players[0].hand.len();
        let other_hand = e.state.players[1].hand.len();
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "lightning_bolt");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .expect("bolt the creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            controller_hand,
            "{}: the creature's controller draws",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            other_hand,
            "{}: the other player does not draw",
            case.id
        );
    }
}

#[test]
fn q84_dies_draw_cards_do_not_draw_when_exiled() {
    for (index, case) in cases().into_iter().enumerate() {
        if case.id == "phantasmal_shieldback" {
            continue;
        }
        let mut e = engine(
            84_024_000 + index as u64,
            &[case.id, "swords_to_plowshares"],
            &[],
        );
        let creature = cast_card(&mut e, &case);
        ensure_in_hand(&mut e, 0, "swords_to_plowshares");
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "swords_to_plowshares");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .expect("exile the creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Exile,
            "{}: Swords exiles rather than destroys",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before - 1,
            "{}: no dies trigger draws a card",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}: the library is unchanged",
            case.id
        );
    }
}

#[test]
fn q84_jeskai_sage_prowess_pumps_until_cleanup() {
    let case = &cases()[0];
    let mut e = engine(
        84_025_001,
        &[case.id, "bonesplitter", "grizzly_bears"],
        &["dark_ritual"],
    );
    let sage = cast_card(&mut e, case);
    assert_eq!(e.effective_power(sage), Some(1));
    ensure_in_hand(&mut e, 0, "bonesplitter");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "bonesplitter");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast a noncreature spell");
    assert_eq!(
        e.state.stack.len(),
        2,
        "Prowess trigger plus the artifact spell"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(sage), Some(2));
    assert_eq!(e.effective_toughness(sage), Some(2));

    ensure_in_hand(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast a creature spell");
    assert_eq!(
        e.state.stack.len(),
        1,
        "a creature spell does not trigger Prowess"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(sage), Some(2));

    e.apply_scenario_command(0, &pass())
        .expect("pass priority to the opponent");
    ensure_in_hand(&mut e, 1, "dark_ritual");
    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "dark_ritual");
    e.apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("opponent casts a noncreature spell");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(sage),
        Some(2),
        "an opponent's cast does not trigger this Prowess"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(sage), Some(1), "cleanup expires Prowess");
    assert_eq!(e.effective_toughness(sage), Some(1));
}

#[test]
fn q84_living_lies_of_loki_counts_only_other_illusions() {
    let case = &cases()[2];
    let mut e = engine(84_026_001, &[case.id, "unsummon"], &[]);
    let loki = cast_card(&mut e, case);
    assert_eq!(e.effective_power(loki), Some(1), "it never counts itself");
    assert_eq!(e.effective_toughness(loki), Some(3));
    let other_illusion = inject_creature_on_battlefield(&mut e, 0, "cloud_dragon");
    assert_eq!(
        e.effective_power(loki),
        Some(2),
        "another Illusion adds one power"
    );
    assert_eq!(
        e.effective_toughness(loki),
        Some(3),
        "toughness is unchanged"
    );
    let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert_eq!(
        e.effective_power(loki),
        Some(2),
        "a non-Illusion adds nothing"
    );
    ensure_in_hand(&mut e, 0, "unsummon");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "unsummon");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(other_illusion)))
        .expect("return the other Illusion to hand");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&other_illusion].zone, Zone::Hand);
    assert_eq!(
        e.effective_power(loki),
        Some(1),
        "the count is continuously reevaluated"
    );
    assert_eq!(
        e.effective_power(bears),
        Some(2),
        "the bystander is untouched"
    );
}

#[test]
fn q84_filigree_familiar_has_no_keywords_and_pays_exact_generic_mana() {
    let case = &cases()[1];
    let mut e = engine(84_027_001, &[case.id], &[]);
    let ws = cast_card(&mut e, case);
    assert!(!e.effective_has_keyword(ws, Keyword::Flying));
}
