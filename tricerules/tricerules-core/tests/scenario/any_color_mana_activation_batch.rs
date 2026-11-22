//! Actual-card scenarios for the queue-v21 any-color mana activation cohort.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3b make these targetless mana
//! abilities resolve immediately without the stack; CR 118.3 requires the printed mana
//! plus tap cost; CR 302.6/602.5a deny a summoning-sick creature's tap cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    activation: ManaGift,
    body: Option<(u32, u32)>,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "celestial_prism",
            activation: ManaGift {
                c: 2,
                ..Default::default()
            },
            body: None,
        },
        Case {
            id: "mana_cylix",
            activation: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: None,
        },
        Case {
            id: "viridian_acolyte",
            activation: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: Some((1, 1)),
        },
    ]
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

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
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

fn offer_each_color(case: &Case, seed: u64) {
    for option in 0u32..5 {
        let mut e = engine(seed + u64::from(option), case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.activation));
        e.apply_scenario_command(0, &mana_command(&e, source, option))
            .expect("activate the selected mana option");
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
fn any_color_mana_activation_batch_canary_cards_offer_all_five_colors() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "viridian_acolyte" | "celestial_prism"))
    {
        offer_each_color(&case, 10_030_000);
    }
}

#[test]
fn any_color_mana_activation_batch_cards_offer_each_of_the_five_colors() {
    for (index, case) in cases().into_iter().enumerate() {
        offer_each_color(&case, 10_030_100 + index as u64 * 10);
    }
}

#[test]
fn any_color_mana_activation_batch_cards_require_the_exact_printed_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_031_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: case.activation.c + 1,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &mana_command(&e, source, 0))
            .expect("activate with one extra mana available");
        let mut expected = [0u32; 6];
        expected[0] = 1;
        expected[5] = 1;
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
            "{}: exactly the printed generic cost is consumed and the chosen color produced",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}", case.id);
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
fn any_color_mana_activation_batch_cards_reject_missing_mana_and_tapped_sources() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_032_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &mana_command(&e, source, 0))
                .is_err(),
            "{}: the printed generic mana is required",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let mut e = engine(10_033_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.activation));
        e.apply_scenario_command(0, &mana_command(&e, source, 0))
            .expect("first activation");
        let command_index = e.state.command_index;
        let before = pool(&e, 0);
        assert!(
            e.apply_scenario_command(0, &mana_command(&e, source, 0))
                .is_err(),
            "{}: a tapped source cannot pay the tap cost",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(pool(&e, 0), before, "{}", case.id);
    }
}

#[test]
fn any_color_mana_activation_batch_cards_credit_the_activating_player_only() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_034_000 + index as u64, case.id);
        end_active_turn(&mut e, 0);
        advance_to_main1_from_game_start(&mut e);
        assert_eq!(e.state.active_player_id(), 1, "{}", case.id);
        let source = inject_permanent_on_battlefield(&mut e, 1, case.id);
        give_mana(&mut e, 1, gift(&case.activation));
        e.apply_scenario_command(1, &mana_command(&e, source, 1))
            .expect("player one activates for blue");
        assert_eq!(pool(&e, 1), (0, 1, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn any_color_mana_activation_batch_viridian_acolyte_needs_to_lose_summoning_sickness() {
    let mut e = engine(10_035_000, "viridian_acolyte");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "viridian_acolyte");
    let slot = hand_index_for_card(&e, 0, "viridian_acolyte");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Viridian Acolyte");
    resolve_entire_stack_two_player(&mut e);
    let source = battlefield_object_for_card(&e, 0, "viridian_acolyte");
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let command_index = e.state.command_index;
    assert!(
        e.apply_scenario_command(0, &mana_command(&e, source, 0))
            .is_err(),
        "CR 302.6: a summoning-sick creature cannot pay a tap cost"
    );
    assert_eq!(e.state.command_index, command_index);
    assert!(!e.state.objects[&source].tapped);
    end_active_turn(&mut e, 0);
    advance_to_main1_from_game_start(&mut e);
    end_active_turn(&mut e, 1);
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &mana_command(&e, source, 0))
        .expect("the next turn the creature may activate");
    assert_eq!(pool(&e, 0), (1, 0, 0, 0, 0, 0));
}
