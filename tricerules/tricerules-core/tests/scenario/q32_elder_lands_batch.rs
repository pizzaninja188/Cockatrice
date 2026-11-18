//! Actual-card scenarios for the queue-v32 Elder land cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3b resolve each
//! targetless mana ability immediately; CR 118.3 requires the tap and mana
//! costs; CR 305.6 leaves the land with no intrinsic mana ability.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    single: char,
    pair: [char; 2],
}

const CASES: [Case; 5] = [
    Case {
        id: "an-havva_township",
        single: 'g',
        pair: ['r', 'w'],
    },
    Case {
        id: "aysen_abbey",
        single: 'w',
        pair: ['g', 'u'],
    },
    Case {
        id: "castle_sengir",
        single: 'b',
        pair: ['u', 'r'],
    },
    Case {
        id: "koskun_keep",
        single: 'r',
        pair: ['b', 'g'],
    },
    Case {
        id: "wizards_school",
        single: 'u',
        pair: ['w', 'b'],
    },
];

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn pool(e: &GameEngine, player: usize) -> ManaPool {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn expected(color: char) -> ManaPool {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn play_land_card(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("play {}: {error:?}", case.id));
    battlefield_object_for_card(e, 0, case.id)
}

fn activate_with_option(
    e: &mut GameEngine,
    source: u32,
    ability: u32,
    option: Option<u32>,
) -> Result<(), tricerules_core::EngineError> {
    let mut command = activate_ability_for(e, source, ability, vec![]);
    if let Some(option) = option {
        let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
            unreachable!()
        };
        activation.mana_option_index = option;
    }
    e.apply_scenario_command(0, &command).map(|_| ())
}

#[test]
fn q32_elder_lands_canary_simplest() {
    let case = &CASES[0];
    let mut e = engine(32_030_001);
    let land = play_land_card(&mut e, case);
    assert!(!e.state.objects[&land].tapped);
    activate_with_option(&mut e, land, 0, None).expect("tap for colorless");
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1));
    assert!(e.state.objects[&land].tapped);

    let single = inject_permanent_on_battlefield(&mut e, 0, case.id);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    activate_with_option(&mut e, single, 1, None).expect("tap for green");
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 1, 1));

    let pair = inject_permanent_on_battlefield(&mut e, 0, case.id);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    activate_with_option(&mut e, pair, 2, Some(0)).expect("tap for red");
    assert_eq!(pool(&e, 0), (0, 0, 0, 1, 1, 1));
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_elder_lands_canary_hardest() {
    let case = &CASES[4];
    let mut e = engine(32_030_002);
    let land = play_land_card(&mut e, case);
    activate_with_option(&mut e, land, 0, None).expect("tap for colorless");
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1));
    let single = inject_permanent_on_battlefield(&mut e, 0, case.id);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    activate_with_option(&mut e, single, 1, None).expect("tap for blue");
    assert_eq!(pool(&e, 0), (0, 1, 0, 0, 0, 1));
}

#[test]
fn q32_elder_lands_cards_offer_each_printed_mana_ability() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_031_000 + index as u64);
        let colorless = play_land_card(&mut e, case);
        activate_with_option(&mut e, colorless, 0, None).expect("colorless");
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);

        let single = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        activate_with_option(&mut e, single, 1, None).expect("single color");
        let mut wanted = expected(case.single);
        wanted.5 = 1;
        assert_eq!(pool(&e, 0), wanted, "{}", case.id);

        for (option, color) in case.pair.into_iter().enumerate() {
            let pair = inject_permanent_on_battlefield(&mut e, 0, case.id);
            let before = pool(&e, 0);
            give_mana(
                &mut e,
                0,
                ManaGift {
                    c: 2,
                    ..Default::default()
                },
            );
            activate_with_option(&mut e, pair, 2, Some(option as u32))
                .unwrap_or_else(|error| panic!("{}: pair: {error:?}", case.id));
            let mut wanted = expected(color);
            wanted.0 += before.0;
            wanted.1 += before.1;
            wanted.2 += before.2;
            wanted.3 += before.3;
            wanted.4 += before.4;
            wanted.5 += before.5;
            assert_eq!(pool(&e, 0), wanted, "{} option {option}", case.id);
        }
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q32_elder_lands_cards_require_the_printed_mana_and_reject_foreign_controllers() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_032_000 + index as u64);
        let source = play_land_card(&mut e, case);
        assert!(
            activate_with_option(&mut e, source, 1, None).is_err(),
            "{}: one generic mana is required",
            case.id
        );
        assert!(
            activate_with_option(&mut e, source, 2, Some(0)).is_err(),
            "{}: two generic mana are required",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);

        let mut foreign = engine(32_033_000 + index as u64);
        let foreign_source = play_land_card(&mut foreign, case);
        foreign
            .apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut foreign, 1, foreign_source, 0, vec![]).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert_eq!(pool(&foreign, 1), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}
