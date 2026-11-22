//! Actual-card scenarios for the reviewed High-Rise Sawjack reach-and-block-pump family.
//!
//! Expectations are printed Oracle values. CR 702.17 gives Reach; CR 509.3b creates
//! one trigger for each attacker-blocker edge; CR 611.2a/514.2 give the pump its
//! until-end-of-turn duration; CR 613.4c applies the layer 7c pump.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::BlockPair;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    pump_power: u32,
    pump_toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "ezuris_archers",
            power: 1,
            toughness: 2,
            pump_power: 3,
            pump_toughness: 0,
        },
        Case {
            id: "high-rise_sawjack",
            power: 2,
            toughness: 3,
            pump_power: 2,
            pump_toughness: 0,
        },
        Case {
            id: "netcaster_spider",
            power: 2,
            toughness: 3,
            pump_power: 2,
            pump_toughness: 0,
        },
        Case {
            id: "woolly_spider",
            power: 2,
            toughness: 3,
            pump_power: 0,
            pump_toughness: 2,
        },
    ]
}

fn setup_block(attacker_card: &str, blocker_card: &str, seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("swamp", &[attacker_card]),
        deck_with("forest", &[blocker_card]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut engine);
    let attacker = relocate_to_battlefield(&mut engine, 0, attacker_card, false);
    let blocker = relocate_to_battlefield(&mut engine, 1, blocker_card, false);

    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare blocker");

    (engine, attacker, blocker)
}

#[test]
fn high_rise_sawjack_batch_cards_pump_when_blocking_a_flyer() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, _, blocker) = setup_block("aven_wind_mage", case.id, 810_000 + index as u64);
        assert_eq!(e.state.stack.len(), 1, "{}: block trigger", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(blocker),
            Some(case.power + case.pump_power),
            "{}: printed pump power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(blocker),
            Some(case.toughness + case.pump_toughness),
            "{}: printed pump toughness",
            case.id
        );
    }
}

#[test]
fn high_rise_sawjack_batch_cards_do_not_trigger_on_a_nonflyer() {
    for (index, case) in cases().into_iter().enumerate() {
        let (e, _, blocker) = setup_block("grizzly_bears", case.id, 811_000 + index as u64);
        assert!(
            e.state.stack.is_empty(),
            "{}: a nonflying attacker does not trigger the block pump",
            case.id
        );
        assert_eq!(e.effective_power(blocker), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(blocker),
            Some(case.toughness),
            "{}",
            case.id
        );
    }
}

#[test]
fn high_rise_sawjack_batch_cards_keep_reach_against_flyers() {
    for (index, case) in cases().into_iter().enumerate() {
        let (e, _, blocker) = setup_block("aven_wind_mage", case.id, 812_000 + index as u64);
        assert!(
            e.effective_has_keyword(blocker, tricerules_cards::Keyword::Reach),
            "{}: printed Reach remains",
            case.id
        );
    }
}
