//! Actual-card scenarios for the reviewed landfall +1/+1 creature family.
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

fn cases() -> [Case; 4] {
    [
        Case {
            id: "hedron_scrabbler",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "planetary_patrol",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "scythe_leopard",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "snapping_gnarlid",
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
fn landfall_pump_remnant_batch_cards_apply_the_printed_effect() {
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
            Some(case.power + 1),
            "{}: landfall pumps power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + 1),
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
fn landfall_pump_remnant_batch_cards_scope_the_trigger_to_the_controller() {
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
fn landfall_pump_remnant_batch_cards_trigger_each_landfall_permanent_and_expire() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(7_032_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let (partner_id, partner_power, partner_toughness) = if case.id == "scythe_leopard" {
            ("snapping_gnarlid", 2, 2)
        } else {
            ("scythe_leopard", 1, 1)
        };
        let partner =
            inject_creature_with_stats(&mut e, 0, partner_id, partner_power, partner_toughness);
        move_ready_to_battlefield(&mut e, 0, "mountain");
        assert!(
            answer_trigger_order_in_engine_order(&mut e),
            "{}: both landfall permanents put a trigger on the stack",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 1),
            "{}: the first land pumps the source",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + 1),
            "{}: the first land pumps the source",
            case.id
        );
        assert_eq!(
            e.effective_power(partner),
            Some(partner_power + 1),
            "{}: each landfall permanent triggers",
            case.id
        );
        assert_eq!(
            e.effective_toughness(partner),
            Some(partner_toughness + 1),
            "{}: each landfall permanent triggers",
            case.id
        );
        move_ready_to_battlefield(&mut e, 0, "mountain");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 2),
            "{}: each land adds a separate +1/+1 trigger",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + 2),
            "{}: each land adds a separate +1/+1 trigger",
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
