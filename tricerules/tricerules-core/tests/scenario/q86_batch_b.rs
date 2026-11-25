//! Actual-card scenarios for the queue-v86 02-crew-vehicles cohort.
//!
//! Five Vehicles whose whole text is one or more printed keywords plus Crew N: Aradara Express
//! ({5}, 8/6, Menace, crew 4), Dragonfly Suit ({2}{W}, 3/2, Flying, crew 1), Ovalchase Dragster
//! ({4}, 6/1, Trample + haste, crew 1), Sky Skiff ({2}, 2/3, Flying, crew 1) and Thundersteel
//! Colossus ({7}, 7/7, Trample + haste, crew 2).
//!
//! Expectations are printed Oracle values. CR 301.7 and 208.3 keep a Vehicle's printed body only
//! while it is a creature; CR 702.122a-e define crew as an activated ability that taps any number
//! of other untapped creatures you control with total power N or greater until end of turn;
//! CR 702.111 (menace), 702.9 (flying), 702.19 (trample) and 702.10 (haste) govern the keywords.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CostObjectRef, CostObjectRefs, CostSelection,
    RuledCommand,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    keywords: &'static [Keyword],
    crew: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "aradara_express",
            mana: ManaGift {
                c: 5,
                ..Default::default()
            },
            body: (8, 6),
            keywords: &[Keyword::Menace],
            crew: 4,
        },
        Case {
            id: "dragonfly_suit",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (3, 2),
            keywords: &[Keyword::Flying],
            crew: 1,
        },
        Case {
            id: "ovalchase_dragster",
            mana: ManaGift {
                c: 4,
                ..Default::default()
            },
            body: (6, 1),
            keywords: &[Keyword::Trample, Keyword::Haste],
            crew: 1,
        },
        Case {
            id: "sky_skiff",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            body: (2, 3),
            keywords: &[Keyword::Flying],
            crew: 1,
        },
        Case {
            id: "thundersteel_colossus",
            mana: ManaGift {
                c: 7,
                ..Default::default()
            },
            body: (7, 7),
            keywords: &[Keyword::Trample, Keyword::Haste],
            crew: 2,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn crew_selection(engine: &GameEngine, objects: &[u32]) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|object_id| CostObjectRef {
                    object_id: *object_id,
                    zone_change_generation: generation(engine, *object_id),
                })
                .collect(),
        })),
    }
}

fn activate_crew(engine: &GameEngine, vehicle: u32, objects: &[u32]) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(vehicle, 0, vec![], vec![crew_selection(engine, objects)]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(engine, vehicle);
    command
}

fn cast_vehicle(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

/// Payer bodies whose total power equals the printed crew threshold exactly, so a RON with a
/// higher or lower crew value cannot pass this suite.
fn crew_payers(e: &mut GameEngine, case: &Case) -> Vec<u32> {
    match case.crew {
        1 => vec![inject_creature_with_stats(e, 0, "grizzly_bears", 1, 1)],
        2 => vec![inject_creature_with_stats(e, 0, "grizzly_bears", 2, 2)],
        4 => vec![
            inject_creature_with_stats(e, 0, "grizzly_bears", 2, 2),
            inject_creature_with_stats(e, 0, "grizzly_bears", 2, 2),
        ],
        other => panic!("unexpected crew threshold {other}"),
    }
}

#[test]
fn q86_vehicle_canary_sky_skiff_crews_into_a_flying_body() {
    let case = &cases()[3];
    assert_eq!(case.id, "sky_skiff");
    let mut e = engine(20_870_100);
    let vehicle = cast_vehicle(&mut e, case);
    let before = e.characteristics(vehicle).expect("characteristics");
    assert!(before.has_type("Artifact") && before.has_type("Vehicle"));
    assert!(
        !before.is_creature(),
        "a Vehicle is not a creature before crew"
    );
    assert_eq!(
        (before.power, before.toughness),
        (None, None),
        "no body before crew"
    );

    let payers = crew_payers(&mut e, case);
    e.apply_scenario_command(0, &activate_crew(&e, vehicle, &payers))
        .expect("crew");
    resolve_entire_stack_two_player(&mut e);
    let crewed = e.characteristics(vehicle).expect("characteristics");
    assert!(crewed.is_creature(), "crewed");
    assert_eq!(
        (crewed.power, crewed.toughness),
        (Some(2), Some(3)),
        "printed body"
    );
    assert!(
        payers.iter().all(|object| e.state.objects[object].tapped),
        "every payer is tapped"
    );
    assert!(
        !e.state.objects[&vehicle].tapped,
        "the Vehicle is not tapped"
    );
    assert!(
        e.effective_has_keyword(vehicle, Keyword::Flying),
        "printed Flying applies once it is a creature"
    );
}

/// Hardest case: Aradara Express has the largest crew threshold and prints Menace.
#[test]
fn q86_vehicle_canary_aradara_express_needs_four_power_and_has_menace() {
    let case = &cases()[0];
    assert_eq!(case.id, "aradara_express");
    let mut e = engine(20_870_101);
    let vehicle = cast_vehicle(&mut e, case);
    let weak = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
    assert!(
        e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[weak]))
            .is_err(),
        "three power does not meet crew 4"
    );
    assert!(
        !e.state.objects[&weak].tapped,
        "a rejected cost pays nothing"
    );
    assert!(e.state.stack.is_empty());
    assert!(
        !e.characteristics(vehicle)
            .expect("characteristics")
            .is_creature(),
        "the Vehicle is not crewed"
    );

    // Exactly four power: 3 + 1.
    let second = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 1, 1);
    e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[weak, second]))
        .expect("crew");
    resolve_entire_stack_two_player(&mut e);
    let crewed = e.characteristics(vehicle).expect("characteristics");
    assert!(crewed.is_creature());
    assert_eq!(
        (crewed.power, crewed.toughness),
        (Some(8), Some(6)),
        "printed body"
    );
    assert!(
        e.effective_has_keyword(vehicle, Keyword::Menace),
        "printed Menace"
    );
    assert!(e.state.objects[&weak].tapped && e.state.objects[&second].tapped);
}

#[test]
fn q86_vehicle_cards_crew_makes_the_printed_artifact_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_871_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, case);
        let payers = crew_payers(&mut e, case);
        e.apply_scenario_command(0, &activate_crew(&e, vehicle, &payers))
            .unwrap_or_else(|error| panic!("{}: crew: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let crewed = e.characteristics(vehicle).expect("characteristics");
        assert!(crewed.has_type("Artifact") && crewed.has_type("Vehicle"));
        assert!(crewed.is_creature(), "{}: crewed", case.id);
        assert_eq!(
            (crewed.power, crewed.toughness),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(vehicle, *keyword),
                "{}: printed {keyword:?}",
                case.id
            );
        }
    }
}

#[test]
fn q86_vehicle_cards_reject_insufficient_power_and_other_creature_crew() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_872_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, case);

        // One power short of the printed threshold. The one-power case must use a 0-power but
        // legal body: a 0/0 would die to CR 704.5f before it could be offered as a payer.
        let short = if case.crew == 1 {
            inject_creature_with_stats(&mut e, 0, "ornithopter", 0, 2)
        } else {
            inject_creature_with_stats(&mut e, 0, "grizzly_bears", case.crew - 1, case.crew - 1)
        };
        assert!(
            e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[short]))
                .is_err(),
            "{}: {} power is below the printed crew {}",
            case.id,
            case.crew - 1,
            case.crew
        );
        assert!(!e.state.objects[&short].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        // An opponent's creature can never pay your crew cost.
        let opponent = inject_creature_with_stats(&mut e, 1, "hill_giant", 5, 5);
        assert!(
            e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[opponent]))
                .is_err(),
            "{}: an opponent's creature cannot pay crew",
            case.id
        );
        assert!(!e.state.objects[&opponent].tapped, "{}", case.id);
        assert!(
            !e.characteristics(vehicle)
                .expect("characteristics")
                .is_creature(),
            "{}: never crewed",
            case.id
        );
    }
}

#[test]
fn q86_vehicle_cards_crew_ends_at_end_of_turn() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_873_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, case);
        let payers = crew_payers(&mut e, case);
        e.apply_scenario_command(0, &activate_crew(&e, vehicle, &payers))
            .expect("crew");
        resolve_entire_stack_two_player(&mut e);
        assert!(e
            .characteristics(vehicle)
            .expect("characteristics")
            .is_creature());
        end_active_turn(&mut e, 0);
        let expired = e.characteristics(vehicle).expect("characteristics");
        assert!(!expired.is_creature(), "{}: crew expired", case.id);
        assert_eq!(
            (expired.power, expired.toughness),
            (None, None),
            "{}: no body after crew expires",
            case.id
        );
    }
}

#[test]
fn q86_vehicle_cards_cast_as_noncreature_artifacts() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_874_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, case);
        let characteristics = e.characteristics(vehicle).expect("characteristics");
        assert!(characteristics.has_type("Artifact"), "{}", case.id);
        assert!(characteristics.has_type("Vehicle"), "{}", case.id);
        assert!(
            !characteristics.is_creature(),
            "{}: not a creature",
            case.id
        );
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (None, None),
            "{}: a noncreature Vehicle has no body",
            case.id
        );
        assert_eq!(
            e.state.objects[&vehicle].owner, e.state.players[0].id,
            "{}: owner",
            case.id
        );
        assert_eq!(
            e.state.objects[&vehicle].controller, e.state.players[0].id,
            "{}: controller",
            case.id
        );
    }
}
