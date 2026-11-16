//! Actual-card scenarios for the queue-v21 tap-and-pay-life mana cohort.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3b make these targetless mana
//! abilities resolve immediately without the stack; CR 118.3 requires every printed
//! cost; CR 119.4 makes the 1 life payment part of the cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    body: Option<(u32, u32)>,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "mana_confluence",
            body: None,
        },
        Case {
            id: "phyrexian_lens",
            body: None,
        },
        Case {
            id: "vesper_ghoul",
            body: Some((1, 1)),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn mana_command(e: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

fn produce_each_color(case: &Case, seed: u64) {
    for option in 0u32..5 {
        let mut e = engine(seed + u64::from(option), case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &mana_command(&e, source, option))
            .expect("activate the life-paying mana ability");
        assert_eq!(
            e.state.players[0].life, 19,
            "{}: exactly one life is paid",
            case.id
        );
        let mut expected = [0u32; 6];
        expected[option as usize] = 1;
        assert_eq!(
            pool(&e, 0),
            (
                expected[0],
                expected[1],
                expected[2],
                expected[3],
                expected[4],
                expected[5]
            ),
            "{} option {option}",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}: mana ability", case.id);
    }
}

#[test]
fn pay_life_any_color_batch_canary_cards_produce_each_color_for_one_life() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "phyrexian_lens" | "vesper_ghoul"))
    {
        produce_each_color(&case, 10_020_000);
    }
}

#[test]
fn pay_life_any_color_batch_cards_produce_each_of_the_five_colors() {
    for (index, case) in cases().into_iter().enumerate() {
        produce_each_color(&case, 10_020_100 + index as u64 * 10);
    }
}

#[test]
fn pay_life_any_color_batch_cards_pay_exactly_one_life_and_the_tap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_021_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let command = mana_command(&e, source, 4);
        e.apply_scenario_command(0, &command)
            .expect("activate for green");
        assert_eq!(e.state.players[0].life, 19, "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 1, 0), "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &command).is_err(),
            "{}: the tapped source cannot be activated again",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(e.state.players[0].life, 19, "{}", case.id);
        if let Some((power, toughness)) = case.body {
            assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                (Some(power), Some(toughness)),
                "{}: printed body",
                case.id
            );
        }
    }
}

#[test]
fn pay_life_any_color_batch_cards_reject_activation_at_zero_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_022_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.state.players[0].life = 0;
        let before = pool(&e, 0);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &mana_command(&e, source, 0))
                .is_err(),
            "{}: a player at zero life cannot pay the cost",
            case.id
        );
        assert_eq!(e.state.players[0].life, 0, "{}", case.id);
        assert_eq!(pool(&e, 0), before, "{}", case.id);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn pay_life_any_color_batch_cards_credit_the_activating_player_only() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_023_000 + index as u64, case.id);
        end_active_turn(&mut e, 0);
        advance_to_main1_from_game_start(&mut e);
        assert_eq!(e.state.active_player_id(), 1, "{}", case.id);
        let source = inject_permanent_on_battlefield(&mut e, 1, case.id);
        e.apply_scenario_command(1, &mana_command(&e, source, 2))
            .expect("player one activates for black");
        assert_eq!(e.state.players[1].life, 19, "{}", case.id);
        assert_eq!(pool(&e, 1), (0, 0, 1, 0, 0, 0), "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}

#[test]
fn pay_life_any_color_batch_vesper_ghoul_needs_to_lose_summoning_sickness() {
    let mut e = engine(10_024_000, "vesper_ghoul");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "vesper_ghoul");
    let slot = hand_index_for_card(&e, 0, "vesper_ghoul");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Vesper Ghoul");
    resolve_entire_stack_two_player(&mut e);
    let ghoul = battlefield_object_for_card(&e, 0, "vesper_ghoul");
    assert_eq!(e.state.objects[&ghoul].zone, Zone::Battlefield);
    let command_index = e.state.command_index;
    assert!(
        e.apply_scenario_command(0, &mana_command(&e, ghoul, 0))
            .is_err(),
        "CR 302.6: a summoning-sick creature cannot pay a tap cost"
    );
    assert_eq!(e.state.command_index, command_index);
    assert_eq!(e.state.players[0].life, 20);
    assert!(!e.state.objects[&ghoul].tapped);
    end_active_turn(&mut e, 0);
    advance_to_main1_from_game_start(&mut e);
    end_active_turn(&mut e, 1);
    advance_to_main1_from_game_start(&mut e);
    e.apply_scenario_command(0, &mana_command(&e, ghoul, 0))
        .expect("the next turn the creature may activate");
    assert_eq!(e.state.players[0].life, 19);
}
