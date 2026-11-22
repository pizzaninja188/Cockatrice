//! Actual-card scenarios for the queue-v27 enters-tap-freeze cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a triggers on entry; CR 115.1
//! makes the chosen creature a target; CR 611.3 and 502.3 make it skip its
//! controller's next untap step. The pinned rulings confirm that an already-tapped
//! target still skips that step.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 5] {
    [
        Case { id: "frost_lynx" },
        Case {
            id: "kor_hookmaster",
        },
        Case {
            id: "watertrap_weaver",
        },
        Case {
            id: "frost_trickster",
        },
        Case { id: "spire_patrol" },
    ]
}

fn choose_trigger_target(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn enter_and_target(e: &mut GameEngine, case: &Case, target: u32) {
    let source = move_ready_to_battlefield(e, 0, case.id);
    assert!(
        e.state.objects[&source].zone == Zone::Battlefield,
        "{}",
        case.id
    );
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{}: one entry trigger",
        case.id
    );
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .unwrap_or_else(|error| panic!("{}: choose target: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    assert!(e.state.pending_triggers.is_empty(), "{}", case.id);
}

#[test]
fn etb_freeze_batch_cards_tap_the_chosen_opponent_creature_and_skip_its_next_untap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_060_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        enter_and_target(&mut e, &case, target);
        assert!(e.state.objects[&target].tapped, "{}: tapped", case.id);
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.active_player_id(), 1, "{}", case.id);
        assert!(
            e.state.objects[&target].tapped,
            "{}: skips its controller's next untap step",
            case.id
        );
        advance_to_main1_from_game_start(&mut e);
        end_active_turn(&mut e, 1);
        assert_eq!(e.state.active_player_id(), 0, "{}", case.id);
        advance_to_main1_from_game_start(&mut e);
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.active_player_id(), 1, "{}", case.id);
        assert!(
            !e.state.objects[&target].tapped,
            "{}: untaps in the following untap step",
            case.id
        );
    }
}

#[test]
fn etb_freeze_batch_cards_still_skip_an_already_tapped_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_061_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&target).expect("target").tapped = true;
        enter_and_target(&mut e, &case, target);
        assert!(e.state.objects[&target].tapped, "{}", case.id);
        end_active_turn(&mut e, 0);
        assert!(
            e.state.objects[&target].tapped,
            "{}: an already-tapped target still skips the step",
            case.id
        );
    }
}

#[test]
fn etb_freeze_batch_cards_only_target_an_opponents_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_062_000 + index as u64, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let _source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        assert!(
            e.apply_scenario_command(0, &choose_trigger_target(target_object(own)))
                .is_err(),
            "{}: a creature you control is not a legal target",
            case.id
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_object(opponent)))
            .expect("choose the opponent's creature");
        resolve_entire_stack_two_player(&mut e);
        assert!(!e.state.objects[&own].tapped, "{}", case.id);
        assert!(e.state.objects[&opponent].tapped, "{}", case.id);
    }
}

#[test]
fn etb_freeze_batch_cards_fizzle_when_the_chosen_creature_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_063_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
            .expect("choose the target");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(e.state.pending_triggers.is_empty(), "{}", case.id);
        assert!(
            e.state.objects[&source].zone == Zone::Battlefield,
            "{}: the source remains",
            case.id
        );
    }
}
