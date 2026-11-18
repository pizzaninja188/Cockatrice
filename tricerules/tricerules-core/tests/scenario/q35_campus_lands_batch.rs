//! Actual-card scenarios for the queue-v35 campus scry land cohort.
//!
//! Expectations are printed Oracle values. CR 614.1d makes the land enter
//! tapped, CR 605.1a/605.3b resolve the mana ability immediately, and
//! CR 701.22a-b govern the four-mana scry 1 activation.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    first: u32,
    second: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "lorehold_campus",
            first: 3,
            second: 0,
        },
        Case {
            id: "prismari_campus",
            first: 1,
            second: 3,
        },
        Case {
            id: "quandrix_campus",
            first: 4,
            second: 1,
        },
        Case {
            id: "silverquill_campus",
            first: 0,
            second: 2,
        },
        Case {
            id: "witherbloom_campus",
            first: 2,
            second: 4,
        },
    ]
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn expected(index: u32) -> (u32, u32, u32, u32, u32, u32) {
    let mut values = [0u32; 6];
    values[index as usize] = 1;
    (
        values[0], values[1], values[2], values[3], values[4], values[5],
    )
}

fn engine(seed: u64, card: &str) -> GameEngine {
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

fn mana_command(e: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect::<Vec<_>>();
    e.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        e.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

#[test]
fn q35_campus_lands_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(35_060_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(
        e.state.objects[&source].tapped,
        "Quandrix Campus enters tapped"
    );
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    e.apply_scenario_command(0, &mana_command(&e, source, 0))
        .expect("activate the first mana option");
    assert_eq!(pool(&e), expected(case.first), "first printed color");
}

#[test]
fn q35_campus_lands_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(35_060_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    e.apply_scenario_command(0, &mana_command(&e, source, 1))
        .expect("activate the second mana option");
    assert_eq!(pool(&e), expected(case.second), "second printed color");
}

#[test]
fn q35_campus_lands_cards_enter_tapped_and_produce_the_printed_pair() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_061_000 + index as u64, case.id);
        let entered = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            e.state.objects[&entered].tapped,
            "{}: enters tapped",
            case.id
        );
        assert_eq!(
            (e.effective_power(entered), e.effective_toughness(entered)),
            (None, None),
            "{}: a land has no body",
            case.id
        );
        for (option, color) in [case.first, case.second].into_iter().enumerate() {
            let mut e = engine(35_061_100 + index as u64 * 10 + option as u64, case.id);
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            e.apply_scenario_command(0, &mana_command(&e, source, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(
                pool(&e),
                expected(color),
                "{}: option {option} produces exactly one mana",
                case.id
            );
            assert!(
                e.state.objects[&source].tapped,
                "{}: the mana ability taps the land",
                case.id
            );
            assert!(e.state.stack.is_empty(), "{}: mana ability", case.id);
        }
    }
}

#[test]
fn q35_campus_lands_cards_scry_for_four_mana_and_a_tap() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_062_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let seated = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
        let scried = seated[0];
        let next = seated[1];
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 4,
                ..Default::default()
            },
        );
        apply_ability(&mut e, 0, source, 1, vec![])
            .unwrap_or_else(|error| panic!("{}: scry activation: {error:?}", case.id));
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: four generic mana",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
        let batch = pass_both_players_with_batch(&mut e);
        let choice = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice.clone()),
                _ => None,
            })
            .expect("scry 1 parks for a choice");
        assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
        assert_eq!((choice.min, choice.max), (0, 1), "{}: scry 1", case.id);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![scried]))
            .expect("bottom the scried card");
        assert_eq!(
            *e.state.players[0].library.back().expect("library"),
            scried,
            "{}: the bottomed card is last",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.front().copied(),
            Some(next),
            "{}: the next card is now on top",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}
