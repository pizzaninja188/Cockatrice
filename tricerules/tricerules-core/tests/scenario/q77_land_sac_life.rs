//! Actual-card scenarios for the queue-v77 02-land-sac-life cohort.
//!
//! Expectations are printed Oracle values. CR 602.2b and 701.21a make the land sacrifice an
//! activation cost paid before the ability resolves; CR 118.3 forbids an unpayable cost and
//! CR 119.3 applies the printed life gain to the activating player when the ability resolves.
//! No ability targets a player or permanent.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, RuledCommand};

struct Case {
    id: &'static str,
    life: u32,
    land: &'static str,
    rejects: Option<&'static str>,
}

const CASES: [Case; 3] = [
    Case {
        id: "zuran_orb",
        life: 2,
        land: "forest",
        rejects: None,
    },
    Case {
        id: "overgrown_estate",
        life: 3,
        land: "island",
        rejects: None,
    },
    Case {
        id: "dark_heart_of_the_wood",
        life: 3,
        land: "forest",
        rejects: Some("island"),
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("forest", &[])]);
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

fn activate_with_sacrifice(e: &GameEngine, source: u32, land: u32) -> RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("constructed an ability activation")
    };
    activation.cost_selections = vec![permanent_cost_selection(0, land)];
    command
}

#[test]
fn q77_land_sac_life_canary_simplest_zuran_orb_sacrifices_a_land_for_two_life() {
    let case = &CASES[0];
    let mut e = engine(77_200_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let land = inject_permanent_on_battlefield(&mut e, 0, case.land);
    let command = activate_with_sacrifice(&e, source, land);

    e.apply_scenario_command(0, &command)
        .expect("activate Zuran Orb");
    assert_eq!(
        e.state.objects[&land].zone,
        Zone::Graveyard,
        "the land is sacrificed as a cost"
    );
    assert!(!e.state.objects[&source].tapped, "the Orb has no tap cost");
    assert_eq!(e.state.players[0].life, 20, "life waits for resolution");
    assert_eq!(e.state.players[1].life, 20);
    assert_eq!(e.state.stack.len(), 1, "the ability uses the stack");

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 22, "exactly two life");
    assert_eq!(
        e.state.players[1].life, 20,
        "only the controller gains life"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q77_land_sac_life_canary_hardest_dark_heart_requires_a_forest_and_gains_three() {
    let case = &CASES[2];
    let mut e = engine(77_200_002);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    e.apply_scenario_command(0, &activate_with_sacrifice(&e, source, forest))
        .expect("sacrifice a Forest");
    assert_eq!(e.state.objects[&forest].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 23, "exactly three life");

    let mut rejected = engine(77_200_003);
    let source = inject_permanent_on_battlefield(&mut rejected, 0, case.id);
    let island = inject_permanent_on_battlefield(&mut rejected, 0, "island");
    assert!(
        rejected
            .apply_scenario_command(0, &activate_with_sacrifice(&rejected, source, island))
            .is_err(),
        "an Island cannot pay a Forest sacrifice cost"
    );
    assert_eq!(
        rejected.state.objects[&island].zone,
        Zone::Battlefield,
        "the rejected activation sacrifices nothing"
    );
    assert_eq!(rejected.state.players[0].life, 20);
    assert!(rejected.state.stack.is_empty());
}

#[test]
fn q77_land_sac_life_cards_use_their_printed_costs_and_life_totals() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(77_201_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.land);
        e.apply_scenario_command(0, &activate_with_sacrifice(&e, source, land))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Graveyard,
            "{}: the chosen land is the cost",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: pre-resolution", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life,
            20 + case.life as i32,
            "{}: exactly the printed life gain",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q77_land_sac_life_cards_reject_nonmatching_or_missing_land_costs_atomically() {
    for (index, case) in CASES.iter().enumerate() {
        let Some(rejected_land) = case.rejects else {
            continue;
        };
        let mut e = engine(77_202_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, rejected_land);
        assert!(
            e.apply_scenario_command(0, &activate_with_sacrifice(&e, source, land))
                .is_err(),
            "{}: {rejected_land} is not a legal sacrifice selection",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}: rejected cost sacrifices nothing",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }

    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(77_202_050 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let nonland = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert!(
            e.apply_scenario_command(0, &activate_with_sacrifice(&e, source, nonland))
                .is_err(),
            "{}: a nonland permanent cannot pay a land sacrifice cost",
            case.id
        );
        assert_eq!(
            e.state.objects[&nonland].zone,
            Zone::Battlefield,
            "{}: rejected nonland cost sacrifices nothing",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }

    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(77_202_100 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let command = activate_ability_for(&e, source, 0, vec![]);
        assert!(
            e.apply_scenario_command(0, &command).is_err(),
            "{}: the sacrifice cost must be supplied",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
