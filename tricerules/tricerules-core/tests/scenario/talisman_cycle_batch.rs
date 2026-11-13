//! Actual-card scenarios for the reviewed Talisman artifact cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3a-b make the targetless mana abilities resolve immediately; CR 120.3a deals the printed one damage to the ability's controller; CR 118.3 requires the tap cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    first: char,
    second: char,
}

fn cases() -> [Case; 9] {
    [
        Case {
            id: "talisman_of_conviction",
            first: 'r',
            second: 'w',
        },
        Case {
            id: "talisman_of_creativity",
            first: 'u',
            second: 'r',
        },
        Case {
            id: "talisman_of_curiosity",
            first: 'g',
            second: 'u',
        },
        Case {
            id: "talisman_of_dominance",
            first: 'u',
            second: 'b',
        },
        Case {
            id: "talisman_of_hierarchy",
            first: 'w',
            second: 'b',
        },
        Case {
            id: "talisman_of_indulgence",
            first: 'b',
            second: 'r',
        },
        Case {
            id: "talisman_of_progress",
            first: 'w',
            second: 'u',
        },
        Case {
            id: "talisman_of_resilience",
            first: 'b',
            second: 'g',
        },
        Case {
            id: "talisman_of_unity",
            first: 'g',
            second: 'w',
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

fn cast_talisman(e: &mut GameEngine, card: &str) -> u32 {
    give_mana(
        e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = inject_card_into_hand(e, 0, card);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{card}: resolves"
    );
    battlefield_object_for_card(e, 0, card)
}

fn command(
    e: &GameEngine,
    source: u32,
    ability: u32,
    option: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(e, source, ability, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for builds an ActivateAbility command")
    };
    activation.mana_option_index = option;
    command
}

fn try_activate(e: &mut GameEngine, source: u32, ability: u32, option: u32) -> bool {
    e.apply_scenario_command(0, &command(e, source, ability, option))
        .is_ok()
}

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn mask(color: char) -> (u32, u32, u32, u32, u32, u32) {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
}

#[test]
fn talisman_cycle_batch_cards_add_colorless_or_each_color_and_deal_one_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut colorless = engine(11_300_000 + index as u64, case.id);
        let source = cast_talisman(&mut colorless, case.id);
        colorless
            .apply_scenario_command(0, &command(&colorless, source, 0, 0))
            .expect("activate colorless mana");
        assert_eq!(pool(&colorless, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);
        assert_eq!(
            colorless.state.players[0].life, 20,
            "{}: no self damage",
            case.id
        );
        assert!(colorless.state.stack.is_empty(), "{}", case.id);

        for (offset, (option, color)) in [(0, case.first), (1, case.second)].into_iter().enumerate()
        {
            let mut e = engine(11_310_000 + index as u64 * 10 + offset as u64, case.id);
            let source = cast_talisman(&mut e, case.id);
            e.apply_scenario_command(0, &command(&e, source, 1, option))
                .expect("activate colored mana");
            assert_eq!(pool(&e, 0), mask(color), "{} option {option}", case.id);
            assert_eq!(
                e.state.players[0].life, 19,
                "{} option {option}: one self damage",
                case.id
            );
            assert_eq!(
                e.state.players[1].life, 20,
                "{}: opponent undamaged",
                case.id
            );
            assert!(e.state.objects[&source].tapped, "{}", case.id);
            assert!(
                e.state.stack.is_empty(),
                "{}: immediate mana ability",
                case.id
            );
        }
    }
}

#[test]
fn talisman_cycle_batch_cards_require_tap_and_untapped_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_320_000 + index as u64, case.id);
        let source = cast_talisman(&mut e, case.id);
        e.apply_scenario_command(0, &command(&e, source, 0, 0))
            .expect("activate colorless mana");
        assert!(!try_activate(&mut e, source, 0, 0), "{}", case.id);
        assert!(!try_activate(&mut e, source, 1, 0), "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);

        let mut foreign = engine(11_330_000 + index as u64, case.id);
        let foreign_source = cast_talisman(&mut foreign, case.id);
        assert!(
            foreign
                .apply_scenario_command(1, &command(&foreign, foreign_source, 1, 0))
                .is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert_eq!(pool(&foreign, 1), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(foreign.state.players[1].life, 20, "{}", case.id);
        assert!(
            !foreign.state.objects[&foreign_source].tapped,
            "{}",
            case.id
        );
    }
}

#[test]
fn talisman_cycle_batch_cards_do_not_damage_when_activation_is_rejected() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_340_000 + index as u64, case.id);
        let source = cast_talisman(&mut e, case.id);
        e.apply_scenario_command(0, &command(&e, source, 1, 0))
            .expect("activate colored mana");
        assert_eq!(e.state.players[0].life, 19, "{}", case.id);
        assert!(
            !try_activate(&mut e, source, 1, 1),
            "{}: tapped source cannot pay again",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 19,
            "{}: rejected activation deals no damage",
            case.id
        );
        assert_eq!(pool(&e, 0), mask(case.first), "{}", case.id);
    }
}
