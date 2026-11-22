//! Actual-card scenarios for the reviewed landfall +2/+2 creature family.
//!
//! Expectations are printed Oracle values. CR 603.6a makes a land entering under
//! the controller's control trigger landfall; CR 611.2c/613.4c apply the pump to
//! the source only; CR 514.2 ends it at cleanup.
use super::helpers::*;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "akoum_hellhound",
            power: 0,
            toughness: 1,
        },
        Case {
            id: "steppe_lynx",
            power: 0,
            toughness: 1,
        },
        Case {
            id: "valakut_predator",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "canopy_baloth",
            power: 4,
            toughness: 3,
        },
        Case {
            id: "territorial_baloth",
            power: 4,
            toughness: 4,
        },
        Case {
            id: "hedron_rover",
            power: 2,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("mountain", &[]),
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

#[test]
fn landfall_pump_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(7_030_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        let land = move_ready_to_battlefield(&mut e, 0, "mountain");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.players[0].battlefield.contains(&land),
            "{}: the land entered",
            case.id
        );
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 2),
            "{}: landfall pumps power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + 2),
            "{}: landfall pumps toughness",
            case.id
        );
        assert_eq!(
            e.effective_power(bystander),
            Some(2),
            "{}: the pump is source-only",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bystander),
            Some(2),
            "{}: the pump is source-only",
            case.id
        );
    }
}

#[test]
fn landfall_pump_batch_cards_scope_the_trigger_to_the_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(7_031_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let opponent_land = move_ready_to_battlefield(&mut e, 1, "mountain");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.players[1].battlefield.contains(&opponent_land),
            "{}: the opponent's land entered",
            case.id
        );
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: an opponent's land does not trigger this landfall",
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
fn landfall_pump_batch_cards_stack_multiple_triggers_and_expire() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(7_032_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        move_ready_to_battlefield(&mut e, 0, "mountain");
        resolve_entire_stack_two_player(&mut e);
        move_ready_to_battlefield(&mut e, 0, "mountain");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 4),
            "{}: each land adds a separate +2/+2 trigger",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + 4),
            "{}: each land adds a separate +2/+2 trigger",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: CR 514.2 restores power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}: CR 514.2 restores toughness",
            case.id
        );
    }
}
