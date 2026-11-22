//! Actual-card scenarios for the reviewed tapped R/G dual-land family.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a and 605.3b make the
//! targetless mana abilities resolve immediately without using the stack.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

fn cases() -> [&'static str; 3] {
    ["highland_weald", "shivan_oasis", "timber_gorge"]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn timber_gorge_batch_cards_enter_tapped_and_reject_immediate_tap() {
    for (index, id) in cases().into_iter().enumerate() {
        let mut e = engine(840_000 + index as u64, id);
        inject_card_into_hand(&mut e, 0, id);
        let hand_index = hand_index_for_card(&e, 0, id);
        e.apply_scenario_command(0, &play_land(hand_index))
            .expect("play the land");
        let land = battlefield_object_for_card(&e, 0, id);
        assert_eq!(e.state.objects[&land].zone, Zone::Battlefield, "{id}");
        assert!(e.state.objects[&land].tapped, "{id}: enters tapped");
        let before = pool(&e);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
                .is_err(),
            "{id}: a tapped land cannot pay its tap cost"
        );
        assert_eq!(pool(&e), before, "{id}: rejected activation adds no mana");
        assert!(e.state.objects[&land].tapped, "{id}");
    }
}

#[test]
fn timber_gorge_batch_cards_tap_for_each_printed_option_in_order() {
    for (index, id) in cases().into_iter().enumerate() {
        for (option, expected) in [(0u32, (0, 0, 0, 1, 0, 0)), (1u32, (0, 0, 0, 0, 1, 0))] {
            let mut e = engine(841_000 + index as u64 * 10 + u64::from(option), id);
            let land = inject_permanent_on_battlefield(&mut e, 0, id);
            let mut command = activate_ability_for(&e, land, 0, vec![]);
            let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
                unreachable!()
            };
            ability.mana_option_index = option;
            e.apply_scenario_command(0, &command)
                .expect("activate the selected mana option");
            assert_eq!(pool(&e), expected, "{id} option {option}");
            assert!(e.state.objects[&land].tapped, "{id} option {option}");
            assert!(
                e.state.stack.is_empty(),
                "{id}: mana abilities do not use the stack"
            );
        }
    }
}
