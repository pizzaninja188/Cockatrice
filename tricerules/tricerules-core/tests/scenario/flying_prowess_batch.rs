//! Actual-card scenarios for the reviewed Flying + Prowess family.
//!
//! Expectations are printed Oracle values. CR 702.9a-b keep Flying active while
//! CR 702.108a-b make Prowess one controller-relative trigger for each
//! noncreature spell cast; CR 514.2 ends the pump at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "jeskai_windscout",
            power: 2,
            toughness: 1,
        },
        Case {
            id: "lotus_path_djinn",
            power: 2,
            toughness: 3,
        },
        Case {
            id: "niblis_of_dusk",
            power: 2,
            toughness: 1,
        },
        Case {
            id: "riverwheel_aerialists",
            power: 4,
            toughness: 5,
        },
        Case {
            id: "wing_commando",
            power: 2,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                card,
                "bonesplitter",
                "bonesplitter",
                "lightning_bolt",
                "lightning_bolt",
                "grizzly_bears",
            ],
        ),
        deck_with("forest", &["lightning_bolt"]),
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

fn cast_named(e: &mut GameEngine, player: usize, card: &str) {
    ensure_in_hand(e, player, card);
    grant_pool(e, player);
    let slot = hand_index_for_card(e, player, card);
    e.apply_scenario_command(player as i32, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error}"));
}

fn cast_at(e: &mut GameEngine, player: usize, card: &str, target: usize) {
    ensure_in_hand(e, player, card);
    grant_pool(e, player);
    let slot = hand_index_for_card(e, player, card);
    e.apply_scenario_command(
        player as i32,
        &cast_spell(slot, target_player(target as i32)),
    )
    .unwrap_or_else(|error| panic!("cast {card}: {error}"));
}

#[test]
fn flying_prowess_batch_cards_pump_on_noncreature_casts_and_expire() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_050_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            e.effective_has_keyword(source, Keyword::Flying),
            "{}: printed Flying stays active",
            case.id
        );
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );

        cast_named(&mut e, 0, "bonesplitter");
        assert_eq!(
            e.state.stack.len(),
            2,
            "{}: one Prowess trigger plus the artifact spell",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 1),
            "{}: Prowess pumps power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + 1),
            "{}: Prowess pumps toughness",
            case.id
        );

        cast_named(&mut e, 0, "grizzly_bears");
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: a creature spell does not trigger Prowess",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 1),
            "{}",
            case.id
        );

        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: the pump expires at cleanup",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
    }
}

#[test]
fn flying_prowess_batch_cards_ignore_creature_and_opponent_casts() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_051_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &pass())
            .expect("pass priority to the opponent");
        cast_at(&mut e, 1, "lightning_bolt", 0);
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: an opponent's noncreature cast does not trigger this Prowess",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );

        cast_named(&mut e, 0, "grizzly_bears");
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: a creature spell does not trigger Prowess",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
    }
}

#[test]
fn flying_prowess_batch_cards_stack_one_trigger_per_noncreature_spell() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_052_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        cast_at(&mut e, 0, "lightning_bolt", 1);
        assert_eq!(e.state.stack.len(), 2, "{}", case.id);
        cast_at(&mut e, 0, "lightning_bolt", 1);
        assert_eq!(
            e.state.stack.len(),
            4,
            "{}: two noncreature spells make two Prowess triggers",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 2),
            "{}: each Prowess trigger pumps once",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + 2),
            "{}",
            case.id
        );
    }
}
