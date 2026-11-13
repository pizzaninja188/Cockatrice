//! Actual-card scenarios for the reviewed Prowess creature family.
//!
//! Expectations are printed Oracle values. CR 702.108a-b defines Prowess as
//! one controller-relative trigger for each noncreature spell cast; CR 514.2
//! ends its until-end-of-turn pump at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    body: (u32, u32),
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "nimble-blade_khenra",
            body: (1, 3),
        },
        Case {
            id: "bloodfire_expert",
            body: (3, 1),
        },
        Case {
            id: "lightning_visionary",
            body: (2, 1),
        },
        Case {
            id: "vedalken_blademaster",
            body: (2, 3),
        },
        Case {
            id: "jeskai_student",
            body: (1, 3),
        },
        Case {
            id: "umara_entangler",
            body: (2, 1),
        },
        Case {
            id: "sanguinary_mage",
            body: (1, 3),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card, "bonesplitter", "grizzly_bears"]),
        deck_with("forest", &["opt"]),
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

fn cast_spell_named(e: &mut GameEngine, player: usize, card: &str) {
    ensure_in_hand(e, player, card);
    grant_pool(e, player);
    let slot = hand_index_for_card(e, player, card);
    e.apply_scenario_command(player as i32, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error}"));
}

#[test]
fn prowess_creatures_batch_cards_pump_on_noncreature_casts_and_expire() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(880_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);

        cast_spell_named(&mut e, 0, "bonesplitter");
        assert_eq!(
            e.state.stack.len(),
            2,
            "{}: one Prowess trigger plus the artifact spell",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0 + 1),
            "{}: Prowess pumps power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.body.1 + 1),
            "{}: Prowess pumps toughness",
            case.id
        );

        cast_spell_named(&mut e, 0, "grizzly_bears");
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: a creature spell does not trigger Prowess",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0 + 1),
            "{}",
            case.id
        );

        end_active_turn(&mut e, 0);
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
fn prowess_creatures_batch_cards_ignore_creature_and_opponent_casts() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(881_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &pass())
            .expect("pass priority to the opponent");
        cast_spell_named(&mut e, 1, "opt");
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: an opponent's noncreature cast does not trigger this Prowess",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
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
