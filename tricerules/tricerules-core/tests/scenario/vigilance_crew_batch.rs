//! Actual-card scenarios for the queue-v20 vigilance Vehicle cohort.
//!
//! Expectations are printed Oracle values. CR 301.7/208.3 keep a Vehicle's
//! printed body only while it is a creature; CR 702.122a-e define crew; CR
//! 702.20a-b define vigilance; CR 514.2 ends the crewed creature type.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
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

fn cases() -> [Case; 3] {
    [
        Case {
            id: "brute_suit",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 4,
            toughness: 3,
            crew: 1,
        },
        Case {
            id: "dusk_legion_dreadnought",
            mana: ManaGift {
                c: 5,
                ..Default::default()
            },
            power: 4,
            toughness: 6,
            crew: 2,
        },
        Case {
            id: "funeral_longboat",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
            crew: 1,
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
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    slot
}

#[test]
fn vigilance_crew_batch_cards_cast_as_noncreature_artifacts_with_vigilance() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_110_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, &case);
        let characteristics = e.characteristics(vehicle).expect("characteristics");
        assert!(characteristics.has_type("Artifact"), "{}", case.id);
        assert!(characteristics.has_type("Vehicle"), "{}", case.id);
        assert!(
            !characteristics.is_creature(),
            "{}: not a creature",
            case.id
        );
        assert!(
            characteristics.keywords.contains(&Keyword::Vigilance),
            "{}: printed vigilance",
            case.id
        );
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (None, None),
            "{}: a noncreature Vehicle has no body",
            case.id
        );
        assert_eq!(e.state.objects[&vehicle].zone, Zone::Battlefield);
    }
}

#[test]
fn vigilance_crew_batch_cards_crew_makes_a_printed_artifact_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_111_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, &case);
        let payer = if case.crew >= 2 {
            inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3)
        } else {
            inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2)
        };
        e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[payer]))
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
        assert!(e.state.objects[&payer].tapped, "{}: payer tapped", case.id);
        assert!(
            !e.state.objects[&vehicle].tapped,
            "{}: the Vehicle itself is not tapped",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn vigilance_crew_batch_cards_reject_crew_with_insufficient_power() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_112_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, &case);
        let payer = inject_creature_with_stats(&mut e, 0, "ornithopter", 0, 2);
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
fn vigilance_crew_batch_cards_vigilance_vehicle_stays_untapped_when_attacking() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_113_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, &case);
        let payer = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[payer]))
            .expect("crew");
        resolve_entire_stack_two_player(&mut e);
        e.state
            .objects
            .get_mut(&vehicle)
            .expect("vehicle")
            .summoning_sick = false;
        let control = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to begin combat");
        e.apply_scenario_command(0, &pass())
            .expect("ap pass begin combat");
        e.apply_scenario_command(1, &pass())
            .expect("nap pass begin combat");
        e.apply_scenario_command(0, &declare_attackers(vec![vehicle, control]))
            .expect("both creatures attack");
        assert!(
            e.state
                .combat
                .as_ref()
                .expect("combat")
                .attacking
                .contains(&vehicle),
            "{}: the vigilant Vehicle attacks",
            case.id
        );
        assert!(
            !e.state.objects[&vehicle].tapped,
            "{}: CR 702.20b vigilance does not tap it",
            case.id
        );
        assert!(
            e.state.objects[&control].tapped,
            "{}: an ordinary attacker taps",
            case.id
        );
    }
}

#[test]
fn vigilance_crew_batch_cards_crew_ends_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_114_000 + index as u64);
        let vehicle = cast_vehicle(&mut e, &case);
        let payer = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        e.apply_scenario_command(0, &activate_crew(&e, vehicle, &[payer]))
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
