//! Actual-card scenarios for the reviewed Quirion Explorer opponent-land mana family.
//!
//! Expectations are printed Oracle values. CR 106.1a-b/106.5/106.7 govern color
//! filtering and current output; CR 605.1a/605.3b make the targetless ability a mana ability
//! that does not use the stack; CR 302.6 prevents a summoning-sick creature from paying a tap cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    creature: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "fellwar_stone",
            creature: false,
        },
        Case {
            id: "quirion_explorer",
            creature: true,
        },
        Case {
            id: "sylvok_explorer",
            creature: true,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

fn ability_info(e: &mut GameEngine, source: u32) -> tricerules_proto::ruled::v1::AbilityInfo {
    e.initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source)
                .and_then(|object| object.activated_abilities.first())
                .cloned(),
            _ => None,
        })
        .expect("the source publishes its activated mana ability")
}

fn activate_option(
    e: &GameEngine,
    source: u32,
    option: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn quirion_explorer_batch_cards_follow_the_opponents_land_union() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(860_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        inject_permanent_on_battlefield(&mut e, 0, "mountain");
        inject_permanent_on_battlefield(&mut e, 1, "forest");
        inject_permanent_on_battlefield(&mut e, 1, "island");
        let info = ability_info(&mut e, source);
        assert!(info.is_mana_ability, "{}", case.id);
        assert_eq!(info.mana_produced, "U/G", "{}", case.id);
        e.apply_scenario_command(0, &activate_option(&e, source, 0))
            .expect("activate the blue option");
        assert_eq!(pool(&e), (0, 1, 0, 0, 0, 0), "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let mut changed = engine(861_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut changed, 0, case.id);
        inject_permanent_on_battlefield(&mut changed, 1, "island");
        let info = ability_info(&mut changed, source);
        assert_eq!(info.mana_produced, "U", "{}", case.id);
        inject_permanent_on_battlefield(&mut changed, 1, "forest");
        let info = ability_info(&mut changed, source);
        assert_eq!(info.mana_produced, "U/G", "{}", case.id);
    }
}

#[test]
fn quirion_explorer_batch_cards_allow_zero_output_and_reject_an_invalid_option() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(862_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let info = ability_info(&mut e, source);
        assert!(info.is_mana_ability, "{}", case.id);
        assert!(
            info.mana_produced.is_empty(),
            "{}: no opponent lands produce nothing",
            case.id
        );
        e.apply_scenario_command(0, &activate_option(&e, source, 0))
            .expect("zero-output mana ability is still legal");
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);

        let mut e = engine(862_000 + index as u64 + 100, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        inject_permanent_on_battlefield(&mut e, 1, "terrain_generator");
        let info = ability_info(&mut e, source);
        assert!(info.is_mana_ability, "{}", case.id);
        assert!(info.mana_produced.is_empty(), "{}", case.id);
        e.apply_scenario_command(0, &activate_option(&e, source, 0))
            .expect("zero-output mana ability is still legal");
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);

        let mut invalid = engine(863_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut invalid, 0, case.id);
        inject_permanent_on_battlefield(&mut invalid, 1, "forest");
        assert!(
            invalid
                .apply_scenario_command(0, &activate_option(&invalid, source, 1))
                .is_err(),
            "{}: an invalid option index is rejected",
            case.id
        );
        assert!(!invalid.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(pool(&invalid), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(invalid.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn quirion_explorer_batch_cards_ignore_tapped_lands_costs_and_restrictions() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(865_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let forest = inject_permanent_on_battlefield(&mut e, 1, "forest");
        e.state.objects.get_mut(&forest).expect("forest").tapped = true;
        let beacon = inject_permanent_on_battlefield(&mut e, 1, "interplanar_beacon");
        e.state.objects.get_mut(&beacon).expect("beacon").tapped = true;
        let info = ability_info(&mut e, source);
        assert!(info.is_mana_ability, "{}", case.id);
        assert_eq!(
            info.mana_produced, "W/U/B/R/G",
            "{}: tapped lands still contribute, and costs or riders are ignored",
            case.id
        );
        e.apply_scenario_command(0, &activate_option(&e, source, 3))
            .expect("activate the red option");
        assert_eq!(pool(&e), (0, 0, 0, 1, 0, 0), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn quirion_explorer_batch_creatures_respect_summoning_sickness() {
    for (index, case) in cases().into_iter().enumerate() {
        if !case.creature {
            continue;
        }
        let mut e = engine(864_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        inject_permanent_on_battlefield(&mut e, 1, "forest");
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = true;
        assert!(
            e.apply_scenario_command(0, &activate_option(&e, source, 0))
                .is_err(),
            "{}: a summoning-sick creature cannot pay a tap cost",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        e.apply_scenario_command(0, &activate_option(&e, source, 0))
            .expect("next turn the creature taps normally");
        assert_eq!(pool(&e), (0, 0, 0, 0, 1, 0), "{}", case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
