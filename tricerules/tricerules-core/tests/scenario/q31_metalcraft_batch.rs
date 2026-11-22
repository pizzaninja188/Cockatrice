//! Actual-card scenarios for the queue-v31 metalcraft cohort.
//!
//! Expectations are printed Oracle values. CR 611.3/613.4c reevaluate the
//! battlefield aggregate continuously; the source counts itself when it is an
//! artifact creature (CR 109.1/613.4c).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

fn put_card(e: &mut GameEngine, player: i32, card_name: &str, card_id: &str) -> u32 {
    use tricerules_proto::ruled::v1::dev_command::Dev;
    use tricerules_proto::ruled::v1::{DevCommand, DevPutCardInZone, DevZone, RuledCommand};
    e.enable_dev_commands();
    e.apply_scenario_command(
        player,
        &RuledCommand {
            cmd: Some(tricerules_proto::ruled::v1::ruled_command::Cmd::DevCommand(
                DevCommand {
                    target_player_id: player,
                    dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                        card_name: card_name.to_string(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                },
            )),
        },
    )
    .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(e, player as usize, card_id)
}

struct Case {
    id: &'static str,
    name: &'static str,
    mana: ManaGift,
    body: (i64, i64),
    artifact: bool,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "ardent_recruit",
            name: "Ardent Recruit",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            body: (1, 1),
            artifact: false,
        },
        Case {
            id: "carapace_forger",
            name: "Carapace Forger",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            artifact: false,
        },
        Case {
            id: "chrome_steed",
            name: "Chrome Steed",
            mana: ManaGift {
                c: 4,
                ..Default::default()
            },
            body: (2, 2),
            artifact: true,
        },
        Case {
            id: "ghalmas_warden",
            name: "Ghalma's Warden",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 4),
            artifact: false,
        },
        Case {
            id: "razorfield_rhino",
            name: "Razorfield Rhino",
            mana: ManaGift {
                c: 6,
                ..Default::default()
            },
            body: (4, 4),
            artifact: true,
        },
    ]
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

fn add_artifacts(e: &mut GameEngine, count: usize) {
    for _ in 0..count {
        put_card(e, 0, "Bonesplitter", "bonesplitter");
    }
}

#[test]
fn q31_metalcraft_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(31_050_001);
    add_artifacts(&mut e, 3);
    let card = put_card(&mut e, 0, case.name, case.id);
    assert_eq!(e.effective_power(card), Some((case.body.0 + 2) as u32));
    assert_eq!(e.effective_toughness(card), Some((case.body.1 + 2) as u32));
}

#[test]
fn q31_metalcraft_canary_hardest() {
    let case = &cases()[4];
    let mut e = engine(31_050_002);
    add_artifacts(&mut e, 2);
    let card = put_card(&mut e, 0, case.name, case.id);
    assert_eq!(e.effective_power(card), Some((case.body.0 + 2) as u32));
    assert_eq!(e.effective_toughness(card), Some((case.body.1 + 2) as u32));
}

#[test]
fn q31_metalcraft_cards_keep_their_printed_body_below_three_artifacts() {
    for (index, case) in cases().iter().enumerate() {
        let other_artifacts = if case.artifact { 1 } else { 2 };
        let mut e = engine(31_051_000 + index as u64);
        add_artifacts(&mut e, other_artifacts);
        let card = put_card(&mut e, 0, case.name, case.id);
        assert_eq!(
            e.effective_power(card),
            Some(case.body.0 as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(card),
            Some(case.body.1 as u32),
            "{}",
            case.id
        );
    }
}

#[test]
fn q31_metalcraft_cards_gain_two_two_at_three_artifacts() {
    for (index, case) in cases().iter().enumerate() {
        let other_artifacts = if case.artifact { 2 } else { 3 };
        let mut e = engine(31_052_000 + index as u64);
        add_artifacts(&mut e, other_artifacts);
        let card = put_card(&mut e, 0, case.name, case.id);
        assert_eq!(
            e.effective_power(card),
            Some((case.body.0 + 2) as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(card),
            Some((case.body.1 + 2) as u32),
            "{}",
            case.id
        );
    }
}

#[test]
fn q31_metalcraft_cards_count_themselves_when_they_are_artifacts() {
    for (index, case) in cases().iter().enumerate().filter(|(_, case)| case.artifact) {
        let mut e = engine(31_053_000 + index as u64);
        add_artifacts(&mut e, 2);
        let card = put_card(&mut e, 0, case.name, case.id);
        assert_eq!(
            e.effective_power(card),
            Some((case.body.0 + 2) as u32),
            "{}: source plus two other artifacts",
            case.id
        );
    }
}

#[test]
fn q31_metalcraft_cards_reevaluate_after_an_artifact_leaves() {
    for (index, case) in cases().iter().enumerate() {
        let other_artifacts = if case.artifact { 2 } else { 3 };
        let mut e = engine(31_054_000 + index as u64);
        add_artifacts(&mut e, other_artifacts);
        let card = put_card(&mut e, 0, case.name, case.id);
        assert_eq!(
            e.effective_power(card),
            Some((case.body.0 + 2) as u32),
            "{}",
            case.id
        );
        let removed = *e.state.players[0]
            .battlefield
            .iter()
            .find(|oid| e.state.objects[oid].card_id == "bonesplitter")
            .expect("artifact");
        e.state.players[0].battlefield.retain(|oid| *oid != removed);
        let object = e.state.objects.get_mut(&removed).expect("artifact object");
        object.zone = tricerules_core::Zone::Graveyard;
        e.state.players[0].graveyard.push(removed);
        e.initial_response_batch();
        assert_eq!(
            e.effective_power(card),
            Some(case.body.0 as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(card),
            Some(case.body.1 as u32),
            "{}",
            case.id
        );
    }
}

#[test]
fn q31_metalcraft_cards_cast_with_exact_cost() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_055_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}",
            case.id
        );
    }
}
