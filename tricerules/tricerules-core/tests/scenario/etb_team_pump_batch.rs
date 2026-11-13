//! Actual-card scenarios for the reviewed entry team-pump family.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger;
//! CR 611.2c fixes the affected set as the controller's creatures present when
//! the trigger resolves; CR 613.4c and CR 514.2 apply and remove the pump.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "ampryn_tactician",
            power: 3,
            toughness: 3,
        },
        Case {
            id: "civic_stalwart",
            power: 3,
            toughness: 3,
        },
        Case {
            id: "earth-origin_yak",
            power: 2,
            toughness: 4,
        },
        Case {
            id: "leonin_armorguard",
            power: 3,
            toughness: 3,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "grizzly_bears", "grizzly_bears"]),
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

fn cast_named(e: &mut GameEngine, player: usize, card: &str) {
    ensure_in_hand(e, player, card);
    grant_pool(e, player);
    let slot = hand_index_for_card(e, player, card);
    e.apply_scenario_command(player as i32, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error}"));
}

#[test]
fn etb_team_pump_batch_cards_pump_team_on_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_060_000 + index as u64, case.id);
        let early = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
        let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_named(&mut e, 0, case.id);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: the entry trigger waits on the stack",
            case.id
        );
        let source = battlefield_object_for_card(&e, 0, case.id);
        pass_both_players(&mut e);
        assert_eq!(
            e.effective_power(early),
            Some(3),
            "{}: an existing friendly creature is pumped",
            case.id
        );
        assert_eq!(e.effective_toughness(early), Some(3), "{}", case.id);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 1),
            "{}: the source is included",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + 1),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_power(opposing),
            Some(2),
            "{}: an opponent's creature is not pumped",
            case.id
        );
    }
}

#[test]
fn etb_team_pump_batch_cards_scope_pump_to_the_resolving_board() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_061_000 + index as u64, case.id);
        cast_named(&mut e, 0, case.id);
        pass_both_players(&mut e);
        assert_eq!(e.state.stack.len(), 1, "{}", case.id);
        let before_resolution = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        pass_both_players(&mut e);
        assert_eq!(
            e.effective_power(before_resolution),
            Some(3),
            "{}: a creature present at resolution is pumped",
            case.id
        );
        let after_resolution = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(
            e.effective_power(after_resolution),
            Some(2),
            "{}: the pump is not a dynamic anthem",
            case.id
        );
    }
}

#[test]
fn etb_team_pump_batch_cards_expire_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_062_000 + index as u64, case.id);
        let early = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
        cast_named(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(e.effective_power(early), Some(3), "{}", case.id);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 1),
            "{}",
            case.id
        );

        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(early),
            Some(2),
            "{}: the pump expires",
            case.id
        );
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
    }
}
