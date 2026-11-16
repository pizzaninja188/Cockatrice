//! Actual-card scenarios for the reviewed indestructible artifact-land family.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a and 605.3a-b make the
//! dual mana ability resolve immediately; CR 300.2/300.2a make an object with
//! more than one card type combine each type's aspects, and an artifact land is
//! played as a land; CR 702.12b keeps an indestructible permanent alive through
//! damage and destroy effects.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
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
            id: "darkmoss_bridge",
            first: (0, 0, 1, 0, 0, 0),
            second: (0, 0, 0, 0, 1, 0),
        },
        Case {
            id: "drossforge_bridge",
            first: (0, 0, 1, 0, 0, 0),
            second: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "goldmire_bridge",
            first: (1, 0, 0, 0, 0, 0),
            second: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "mistvault_bridge",
            first: (0, 1, 0, 0, 0, 0),
            second: (0, 0, 1, 0, 0, 0),
        },
        Case {
            id: "razortide_bridge",
            first: (1, 0, 0, 0, 0, 0),
            second: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "rustvale_bridge",
            first: (0, 0, 0, 1, 0, 0),
            second: (1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "silverbluff_bridge",
            first: (0, 1, 0, 0, 0, 0),
            second: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "slagwoods_bridge",
            first: (0, 0, 0, 1, 0, 0),
            second: (0, 0, 0, 0, 1, 0),
        },
        Case {
            id: "tanglepool_bridge",
            first: (0, 0, 0, 0, 1, 0),
            second: (0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "thornglint_bridge",
            first: (0, 0, 0, 0, 1, 0),
            second: (1, 0, 0, 0, 0, 0),
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "darkmoss_bridge" | "thornglint_bridge"))
        .collect()
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "shatter"]),
        deck_with("island", &[]),
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

#[test]
fn artifact_lands_bridge_batch_canary_cards_enter_tapped_and_tap_for_both_colors() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(870_000 + index as u64, case.id);
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
            let mut e = engine(871_000 + index as u64 * 10 + u64::from(option), case.id);
            let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
            tap_option(&mut e, land, option, expected, case.id);
        }
    }
}

#[test]
fn artifact_lands_bridge_batch_canary_cards_are_indestructible_artifacts() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(872_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.effective_has_keyword(land, Keyword::Indestructible),
            "{}: printed Indestructible",
            case.id
        );
    }
}

#[test]
fn artifact_lands_bridge_batch_cards_enter_tapped_and_reject_immediate_tap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(873_000 + index as u64, case.id);
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
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn artifact_lands_bridge_batch_cards_tap_for_each_printed_option_in_order() {
    for (index, case) in cases().into_iter().enumerate() {
        for (option, expected) in [(0u32, case.first), (1u32, case.second)] {
            let mut e = engine(874_000 + index as u64 * 10 + u64::from(option), case.id);
            let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
            tap_option(&mut e, land, option, expected, case.id);
        }
    }
}

#[test]
fn artifact_lands_bridge_batch_cards_keep_artifact_land_identity_and_indestructible() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(875_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let characteristics = e.characteristics(land).expect("characteristics");
        assert!(characteristics.is_artifact(), "{}: artifact type", case.id);
        assert!(characteristics.has_type("Land"), "{}: land type", case.id);
        assert!(
            !characteristics.is_creature(),
            "{}: an artifact land is not a creature",
            case.id
        );
        assert!(
            e.effective_has_keyword(land, Keyword::Indestructible),
            "{}: printed Indestructible",
            case.id
        );
    }
}

#[test]
fn artifact_lands_bridge_batch_cards_survive_a_destroy_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(876_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        ensure_in_hand(&mut e, 0, "shatter");
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "shatter");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
            .unwrap_or_else(|error| panic!("{}: cast Shatter at the bridge: {error:?}", case.id));
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact Shatter cost",
            case.id
        );
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}: indestructible survives the destroy effect",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.iter().any(|oid| {
                e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("shatter")
            }),
            "{}: the destroy spell resolved and went to the graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
