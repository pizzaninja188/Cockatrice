//! Actual-card scenarios for the queue-v19 vanilla Vehicle cohort.
//!
//! Expectations are printed Oracle values. CR 301.7 and 208.3 keep a Vehicle's printed
//! body only while it is a creature; CR 702.122a-e define crew as an activated ability
//! that taps any number of other untapped creatures you control with total power N or
//! greater until end of turn.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CostObjectRef, CostObjectRefs, CostSelection,
    RuledCommand,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
    crew: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "consulate_dreadnought",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 7,
            toughness: 11,
            crew: 6,
        },
        Case {
            id: "futurist_sentinel",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            power: 6,
            toughness: 6,
            crew: 3,
        },
        Case {
            id: "irontread_crusher",
            mana: ManaGift {
                c: 4,
                ..Default::default()
            },
            power: 6,
            toughness: 6,
            crew: 3,
        },
        Case {
            id: "sleek_schooner",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 4,
            toughness: 3,
            crew: 1,
        },
    ]
}

fn engine(seed: u64, _card: &str) -> GameEngine {
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
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    slot
}

#[test]
fn vanilla_vehicles_batch_cards_cast_as_noncreature_artifacts() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_000_000 + index as u64, case.id);
        let vehicle = cast_vehicle(&mut e, &case);
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

#[test]
fn vanilla_vehicles_batch_cards_crew_makes_a_printed_artifact_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_001_000 + index as u64, case.id);
        let vehicle = cast_vehicle(&mut e, &case);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let second = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        let third = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let selected: Vec<u32> = if case.crew >= 6 {
            vec![first, second, third]
        } else if case.crew >= 3 {
            vec![second]
        } else {
            vec![first]
        };
        e.apply_scenario_command(0, &activate_crew(&e, vehicle, &selected))
            .unwrap_or_else(|error| panic!("{}: crew: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let crewed = e.characteristics(vehicle).expect("characteristics");
        assert!(crewed.has_type("Artifact") && crewed.has_type("Vehicle"));
        assert!(crewed.is_creature(), "{}: crewed", case.id);
        assert_eq!(
            (crewed.power, crewed.toughness),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert!(
            selected.iter().all(|object| e.state.objects[object].tapped),
            "{}: every selected creature is tapped",
            case.id
        );
        assert!(
            !e.state.objects[&vehicle].tapped,
            "{}: the Vehicle itself is not tapped",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn vanilla_vehicles_batch_cards_crew_taps_only_creatures_you_control() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_002_000 + index as u64, case.id);
        let vehicle = cast_vehicle(&mut e, &case);
        let friendly = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let opponent = inject_creature_with_stats(&mut e, 1, "hill_giant", 5, 5);
        assert!(
            e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[opponent]))
                .is_err(),
            "{}: an opponent's creature cannot pay crew",
            case.id
        );
        assert!(!e.state.objects[&opponent].tapped, "{}", case.id);
        if case.crew <= 2 {
            e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[friendly]))
                .expect("the friendly creature pays crew");
            resolve_entire_stack_two_player(&mut e);
            assert!(e.state.objects[&friendly].tapped, "{}", case.id);
            assert!(!e.state.objects[&opponent].tapped, "{}", case.id);
        }
    }
}

#[test]
fn vanilla_vehicles_batch_cards_reject_crew_with_insufficient_power() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_003_000 + index as u64, case.id);
        let vehicle = cast_vehicle(&mut e, &case);
        let friendly = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let payer = if case.crew > 2 {
            friendly
        } else {
            inject_creature_with_stats(&mut e, 0, "ornithopter", 0, 2)
        };
        assert!(
            e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[payer]))
                .is_err(),
            "{}: insufficient total power",
            case.id
        );
        assert!(!e.state.objects[&payer].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(
            !e.characteristics(vehicle)
                .expect("characteristics")
                .is_creature(),
            "{}: the Vehicle is not crewed",
            case.id
        );
    }
}

#[test]
fn vanilla_vehicles_batch_cards_crew_ends_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_004_000 + index as u64, case.id);
        let vehicle = cast_vehicle(&mut e, &case);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let second = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        let third = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let selected: Vec<u32> = if case.crew >= 6 {
            vec![first, second, third]
        } else if case.crew >= 3 {
            vec![first, third]
        } else {
            vec![first]
        };
        e.apply_scenario_command(0, &activate_crew(&e, vehicle, &selected))
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
fn vanilla_vehicles_batch_cards_crewed_vehicle_can_pay_another_vehicle_crew() {
    let mut e = engine(10_005_000, "sleek_schooner");
    inject_card_into_hand(&mut e, 0, "sleek_schooner");
    inject_card_into_hand(&mut e, 0, "sleek_schooner");
    let first = move_ready_to_battlefield(&mut e, 0, "sleek_schooner");
    let second = move_ready_to_battlefield(&mut e, 0, "sleek_schooner");
    let payer = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    e.apply_scenario_command(0, &activate_crew(&e, first, &[payer]))
        .expect("the first Vehicle is crewed");
    resolve_entire_stack_two_player(&mut e);
    assert!(e
        .characteristics(first)
        .expect("characteristics")
        .is_creature());
    e.apply_scenario_command(0, &activate_crew(&e, second, &[first]))
        .expect("a crewed Vehicle can pay another Vehicle's crew cost");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.objects[&first].tapped,
        "CR 702.122a: the crewed Vehicle was tapped as the second crew's payer"
    );
    assert!(e
        .characteristics(second)
        .expect("characteristics")
        .is_creature());
    assert_eq!(
        (
            e.characteristics(second).expect("characteristics").power,
            e.characteristics(second)
                .expect("characteristics")
                .toughness
        ),
        (Some(4), Some(3)),
        "the second Vehicle has its printed body"
    );
}
