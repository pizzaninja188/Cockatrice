//! Actual-card scenarios for the reviewed investigate dual-land family.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a and 605.3a-b make the
//! dual mana ability resolve immediately; CR 701.16a and 111.10f create one
//! colorless Clue artifact token for each investigate.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    first: (u32, u32, u32, u32, u32, u32),
    second: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "ballroom",
            first: (1, 0, 0, 0, 0, 0),
            second: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "billiard_room",
            first: (0, 0, 1, 0, 0, 0),
            second: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "conservatory",
            first: (0, 0, 0, 0, 1, 0),
            second: (1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "dining_room",
            first: (0, 0, 0, 1, 0, 0),
            second: (0, 0, 0, 0, 1, 0),
        },
        Case {
            id: "hall",
            first: (0, 0, 0, 1, 0, 0),
            second: (1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "kitchen",
            first: (0, 0, 0, 0, 1, 0),
            second: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "library",
            first: (0, 1, 0, 0, 0, 0),
            second: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "lounge",
            first: (0, 0, 1, 0, 0, 0),
            second: (0, 0, 0, 0, 1, 0),
        },
        Case {
            id: "secret_passage",
            first: (0, 1, 0, 0, 0, 0),
            second: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "study",
            first: (1, 0, 0, 0, 0, 0),
            second: (0, 1, 0, 0, 0, 0),
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "study" | "ballroom"))
        .collect()
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

fn tap_option(
    e: &mut GameEngine,
    land: u32,
    option: u32,
    expected: (u32, u32, u32, u32, u32, u32),
    id: &str,
) {
    let mut command = activate_ability_for(e, land, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    e.apply_scenario_command(0, &command)
        .unwrap_or_else(|error| panic!("{id}: activate mana option {option}: {error:?}"));
    assert_eq!(
        pool(e),
        expected,
        "{id} option {option}: exact printed mana"
    );
    assert!(e.state.objects[&land].tapped, "{id} option {option}");
    assert!(
        e.state.stack.is_empty(),
        "{id}: mana abilities do not use the stack"
    );
}

fn investigate(e: &mut GameEngine, land: u32, id: &str) {
    give_mana(
        e,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(e, land, 1, vec![]))
        .unwrap_or_else(|error| panic!("{id}: investigate: {error:?}"));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{id}: investigate costs exactly four generic"
    );
    assert!(
        e.state.objects[&land].tapped,
        "{id}: investigate taps the source"
    );
    let batch = pass_both_players_with_batch(e);
    let created = token_created_events(&batch);
    assert_eq!(created.len(), 1, "{id}: exactly one Clue token");
    let identity = created[0].identity.as_ref().expect("public token identity");
    assert_eq!(identity.name, "Clue", "{id}");
    assert_eq!(
        identity.types,
        ["Artifact", "Clue"],
        "{id}: Clue artifact type"
    );
    assert!(!identity.is_creature, "{id}: a Clue is not a creature");
    assert_eq!(identity.pt, "", "{id}: a Clue has no body");
    assert_eq!(identity.color, "", "{id}: a Clue is colorless");
    assert_eq!(
        e.state.players[1].battlefield.len(),
        0,
        "{id}: the opponent receives no token"
    );
}

#[test]
fn investigate_lands_batch_canary_cards_enter_tapped_and_tap_for_both_colors() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(860_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_index = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(hand_index))
            .expect("play the land");
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
                .is_err(),
            "{}: a tapped land cannot pay its tap cost",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
    for (index, case) in canaries().into_iter().enumerate() {
        for (option, expected) in [(0u32, case.first), (1u32, case.second)] {
            let mut e = engine(861_000 + index as u64 * 10 + u64::from(option), case.id);
            let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
            tap_option(&mut e, land, option, expected, case.id);
        }
    }
}

#[test]
fn investigate_lands_batch_canary_cards_investigate_for_one_clue() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(862_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        investigate(&mut e, land, case.id);
        assert_eq!(
            battlefield_token_oids(&e, 0, "clue").len(),
            1,
            "{}",
            case.id
        );
    }
}

#[test]
fn investigate_lands_batch_cards_enter_tapped_and_reject_immediate_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(863_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_index = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(hand_index))
            .expect("play the land");
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
                .is_err(),
            "{}: a tapped land cannot pay its tap cost",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
                .is_err(),
            "{}: investigate also needs an untapped source",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(
            battlefield_token_oids(&e, 0, "clue").len(),
            0,
            "{}",
            case.id
        );
    }
}

#[test]
fn investigate_lands_batch_cards_tap_for_each_printed_option_in_order() {
    for (index, case) in cases().into_iter().enumerate() {
        for (option, expected) in [(0u32, case.first), (1u32, case.second)] {
            let mut e = engine(864_000 + index as u64 * 10 + u64::from(option), case.id);
            let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
            tap_option(&mut e, land, option, expected, case.id);
        }
    }
}

#[test]
fn investigate_lands_batch_cards_create_one_clue_per_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(865_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        investigate(&mut e, land, case.id);
        assert_eq!(
            battlefield_token_oids(&e, 0, "clue").len(),
            1,
            "{}",
            case.id
        );
        let second = inject_permanent_on_battlefield(&mut e, 0, case.id);
        investigate(&mut e, second, case.id);
        assert_eq!(
            battlefield_token_oids(&e, 0, "clue").len(),
            2,
            "{}: the ability is repeatable",
            case.id
        );
    }
}

#[test]
fn investigate_lands_batch_cards_reject_investigate_without_four_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(866_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 3,
                ..Default::default()
            },
        );
        let before = pool(&e);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
                .is_err(),
            "{}: investigate needs four generic mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert!(
            !e.state.objects[&land].tapped,
            "{}: the source is untapped",
            case.id
        );
        assert_eq!(
            battlefield_token_oids(&e, 0, "clue").len(),
            0,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
