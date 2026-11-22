//! Actual-card scenarios for the queue-v28 team +1/+1 counter cohort.
//!
//! Expectations are printed Oracle values. CR 608.2c applies the single instruction
//! to the creatures controlled at resolution; CR 122.1 and 613.4c make each counter a
//! permanent +1/+1 modification. The opponent's creatures are outside the filter.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "basris_solidarity",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "titanias_boon",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "inspiring_roar",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "grizzly_bears", "savannah_lions"]),
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

fn cast_team_counters(e: &mut GameEngine, case: &Case) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
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
        "{}: exact cast cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn team_counters_batch_cards_add_one_counter_to_each_controlled_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_030_000 + index as u64, case.id);
        let first = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let second = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        let opposing = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");

        cast_team_counters(&mut e, &case);

        assert_eq!(
            (e.effective_power(first), e.effective_toughness(first)),
            (Some(3), Some(3)),
            "{}: first controlled creature",
            case.id
        );
        assert_eq!(
            (e.effective_power(second), e.effective_toughness(second)),
            (Some(3), Some(2)),
            "{}: second controlled creature",
            case.id
        );
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (Some(2), Some(2)),
            "{}: the opponent's creature is not affected",
            case.id
        );
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, case.id),
            1,
            "{}: the sorcery is in its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn team_counters_batch_cards_do_not_affect_creatures_that_arrive_later() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_031_000 + index as u64, case.id);
        let first = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        cast_team_counters(&mut e, &case);
        assert_eq!(
            (e.effective_power(first), e.effective_toughness(first)),
            (Some(3), Some(3)),
            "{}",
            case.id
        );

        let later = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        assert_eq!(
            (e.effective_power(later), e.effective_toughness(later)),
            (Some(2), Some(1)),
            "{}: a later entrant is not pumped",
            case.id
        );
        assert_eq!(
            e.state.objects[&later].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
