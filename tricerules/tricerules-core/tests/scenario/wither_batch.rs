//! Actual-card scenarios for the reviewed Wither cohort.
//!
//! Expectations are printed Oracle values. CR 702.80a replaces damage marked on
//! a creature with that many -1/-1 counters; damage dealt to a player is still
//! ordinary life loss.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    power: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "harvest_gwyllion",
            power: 2,
        },
        Case {
            id: "scuzzback_scrapper",
            power: 1,
        },
        Case {
            id: "sickle_ripper",
            power: 2,
        },
        Case {
            id: "smoldering_butcher",
            power: 4,
        },
        Case {
            id: "wildslayer_elves",
            power: 3,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card]),
        deck_with("plains", &["wall_of_stone"]),
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
    advance_to_declare_attackers(&mut e);
    e
}

#[test]
fn wither_batch_cards_have_wither() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_240_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            e.effective_has_keyword(source, tricerules_cards::Keyword::Wither),
            "{}: has Wither",
            case.id
        );
    }
}

#[test]
fn wither_batch_cards_place_minus_counters_instead_of_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_241_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let blocker = move_ready_to_battlefield(&mut e, 1, "wall_of_stone");
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the wither attacker");
        pass_both_players(&mut e);
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("block with Wall of Stone");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&blocker].counter_count(CounterKind::MinusOneMinusOne),
            case.power,
            "{}: one -1/-1 counter per damage",
            case.id
        );
        assert_eq!(
            e.state.objects[&blocker].damage, 0,
            "{}: wither does not mark damage",
            case.id
        );
        assert_eq!(
            e.state.objects[&attacker].zone,
            Zone::Battlefield,
            "{}: Wall of Stone deals no damage",
            case.id
        );
        assert_eq!(
            e.effective_toughness(blocker),
            Some(8 - case.power),
            "{}: counters reduce toughness",
            case.id
        );
    }
}

#[test]
fn wither_batch_cards_deal_normal_damage_to_players() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_242_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the wither attacker");
        pass_both_players(&mut e);
        e.apply_scenario_command(1, &declare_blockers(vec![]))
            .expect("declare no blockers");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.power).expect("power fits"),
            "{}: unblocked wither damage is ordinary life loss",
            case.id
        );
    }
}
