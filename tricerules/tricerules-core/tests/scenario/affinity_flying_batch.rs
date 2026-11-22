//! Actual-card scenarios for the queue-v18 affinity-for-artifacts flier cohort.
//!
//! Expectations are printed Oracle values. CR 702.41a reduces only the generic
//! component by one for each artifact its controller controls; CR 601.2f applies
//! the reduction while the spell is on the stack; CR 702.9a-b grants flying.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    full_mana: ManaGift,
    reduced_two_mana: ManaGift,
    colored_only: ManaGift,
    generic: u32,
    body: (u32, u32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "qumulox",
            full_mana: ManaGift {
                u: 2,
                c: 6,
                ..Default::default()
            },
            reduced_two_mana: ManaGift {
                u: 2,
                c: 4,
                ..Default::default()
            },
            colored_only: ManaGift {
                u: 2,
                ..Default::default()
            },
            generic: 6,
            body: (5, 4),
        },
        Case {
            id: "somber_hoverguard",
            full_mana: ManaGift {
                u: 1,
                c: 5,
                ..Default::default()
            },
            reduced_two_mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            colored_only: ManaGift {
                u: 1,
                ..Default::default()
            },
            generic: 5,
            body: (3, 2),
        },
        Case {
            id: "utrom_monitor",
            full_mana: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
            reduced_two_mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            colored_only: ManaGift {
                u: 1,
                ..Default::default()
            },
            generic: 4,
            body: (3, 3),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn generic_cost_reduction(e: &mut GameEngine, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(e, 0, card_id) as u32;
    let batch = e.initial_response_batch();
    batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| panic!("missing cast action for {card_id}"))
        .generic_cost_reduction
}

fn inject_artifacts(e: &mut GameEngine, player: usize, count: usize) {
    for index in 0..count {
        let card = if index % 2 == 0 {
            "ornithopter"
        } else {
            "short_sword"
        };
        inject_permanent_on_battlefield(e, player, card);
    }
}

#[test]
fn affinity_flying_batch_cards_cast_with_exact_characteristics_and_flying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_880_000 + index as u64);
        give_mana(&mut e, 0, gift(&case.full_mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        assert_eq!(
            generic_cost_reduction(&mut e, case.id),
            0,
            "{}: no artifacts means no reduction",
            case.id
        );
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact full cost", case.id);
        resolve_entire_stack_two_player(&mut e);
        let permanent = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (
                e.effective_power(permanent),
                e.effective_toughness(permanent)
            ),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(permanent, Keyword::Flying),
            "{}: flying",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Battlefield,
            "{}: the spell resolves as a permanent",
            case.id
        );
    }
}

#[test]
fn affinity_flying_batch_cards_reduce_only_for_artifacts_you_control() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_881_000 + index as u64);
        inject_artifacts(&mut e, 0, 2);
        inject_artifacts(&mut e, 1, 3);
        inject_card_into_hand(&mut e, 0, case.id);
        assert_eq!(
            generic_cost_reduction(&mut e, case.id),
            2,
            "{}: only the controller's two artifacts reduce the generic cost",
            case.id
        );
        give_mana(&mut e, 0, gift(&case.reduced_two_mana));
        e.apply_scenario_command(0, &cast_spell(hand_index_for_card(&e, 0, case.id), vec![]))
            .unwrap_or_else(|error| panic!("{}: reduced cast: {error:?}", case.id));
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: the colored pips and reduced generic cost were paid exactly",
            case.id
        );
    }
}

#[test]
fn affinity_flying_batch_cards_floor_the_generic_cost_at_zero_and_keep_colored_pips() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_882_000 + index as u64);
        inject_artifacts(&mut e, 0, case.generic as usize + 2);
        inject_card_into_hand(&mut e, 0, case.id);
        assert_eq!(
            generic_cost_reduction(&mut e, case.id),
            case.generic + 2,
            "{}: every controlled artifact reduces the generic component",
            case.id
        );
        give_mana(&mut e, 0, gift(&case.colored_only));
        e.apply_scenario_command(0, &cast_spell(hand_index_for_card(&e, 0, case.id), vec![]))
            .unwrap_or_else(|error| panic!("{}: colored-only cast: {error:?}", case.id));
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: only the colored pips remain",
            case.id
        );
    }
}

#[test]
fn affinity_flying_batch_cards_do_not_count_opponents_artifacts_or_the_spell_itself() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_883_000 + index as u64);
        inject_artifacts(&mut e, 1, 5);
        inject_artifacts(&mut e, 0, 1);
        inject_card_into_hand(&mut e, 0, case.id);
        assert_eq!(
            generic_cost_reduction(&mut e, case.id),
            1,
            "{}: only the one artifact you control counts, not the spell in hand",
            case.id
        );
        give_mana(&mut e, 0, gift(&case.colored_only));
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_index_for_card(&e, 0, case.id), vec![]),)
                .is_err(),
            "{}: the unsupported generic remainder rejects the cast",
            case.id
        );
        assert_eq!(
            e.state.players[0].mana_pool.blue, case.colored_only.u,
            "{}: the rejected cast spends no mana",
            case.id
        );
        let remainder = ManaGift {
            c: case.generic - 1,
            ..Default::default()
        };
        give_mana(&mut e, 0, remainder);
        e.apply_scenario_command(0, &cast_spell(hand_index_for_card(&e, 0, case.id), vec![]))
            .unwrap_or_else(|error| panic!("{}: full remainder cast: {error:?}", case.id));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn affinity_flying_batch_cards_leave_hidden_zones_and_life_totals_unchanged() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_884_000 + index as u64);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        give_mana(&mut e, 0, gift(&case.full_mana));
        inject_card_into_hand(&mut e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_index_for_card(&e, 0, case.id), vec![]))
            .expect("cast");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: the opponent's hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: the opponent's library is unchanged",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}
