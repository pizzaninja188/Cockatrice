//! Actual-card scenarios for the queue-v20 tap-to-grant-haste cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 govern the activated
//! ability and cost, CR 701.26 the tap, CR 702.10a-b haste (including the
//! summoning-sickness permission) and CR 514.2 the cleanup expiry.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "akki_drillmaster",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "bloodlust_inciter",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "goblin_motivator",
            power: 1,
            toughness: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &[]),
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

fn activate_haste(e: &mut GameEngine, source: u32, target: u32, id: &str) {
    let command = activate_ability_for(e, source, 0, target_object(target));
    e.apply_scenario_command(0, &command)
        .unwrap_or_else(|error| panic!("{id}: activate haste: {error:?}"));
    assert!(
        e.state.objects[&source].tapped,
        "{id}: tapping the source is a cost"
    );
    assert!(
        !e.effective_has_keyword(target, Keyword::Haste),
        "{id}: the grant is on the stack"
    );
    resolve_entire_stack_two_player(e);
    assert!(
        e.effective_has_keyword(target, Keyword::Haste),
        "{id}: the target gains haste"
    );
}

#[test]
fn tap_grant_haste_batch_cards_grant_haste_after_the_tap_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_300_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate_haste(&mut e, source, target, case.id);
        assert!(
            e.effective_has_keyword(target, Keyword::Haste),
            "{}: an opponent's creature is a legal target",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_grant_haste_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_301_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(artifact)),
            )
            .is_err(),
            "{}: an artifact is not a creature target",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_grant_haste_batch_cards_let_a_summoning_sick_target_attack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_302_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.state
            .objects
            .get_mut(&target)
            .expect("target")
            .summoning_sick = true;
        activate_haste(&mut e, source, target, case.id);
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to begin combat");
        e.apply_scenario_command(0, &pass())
            .expect("ap pass begin combat");
        e.apply_scenario_command(1, &pass())
            .expect("nap pass begin combat");
        e.apply_scenario_command(0, &declare_attackers(vec![target]))
            .expect("haste ignores summoning sickness");
        assert!(
            e.state
                .combat
                .as_ref()
                .expect("combat")
                .attacking
                .contains(&target),
            "{}: the hasty target attacks",
            case.id
        );
    }
}

#[test]
fn tap_grant_haste_batch_cards_expire_haste_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_303_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate_haste(&mut e, source, target, case.id);
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(target, Keyword::Haste),
            "{}: CR 514.2 ends the grant",
            case.id
        );
    }
}

#[test]
fn tap_grant_haste_batch_cards_reject_activation_when_the_source_cannot_tap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_304_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = true;
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(target)),
            )
            .is_err(),
            "{}: a summoning-sick creature cannot pay a tap cost",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert!(
            !e.effective_has_keyword(target, Keyword::Haste),
            "{}: no haste is granted",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
