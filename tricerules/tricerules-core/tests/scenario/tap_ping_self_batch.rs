//! Actual-card scenarios for the reviewed tap-for-two-damage self-damage cohort.
//!
//! Expectations are printed Oracle values. CR 602.2 activates the ability and
//! CR 118.3 requires an untapped source; CR 120.3a applies both damage events.
//! CR 608.2b makes the whole ability fail when its sole target is illegal, so the
//! controller is not damaged in that case.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "goblin_artillery",
        },
        Case {
            id: "orcish_cannoneers",
        },
        Case {
            id: "orcish_artillery",
        },
    ]
}

fn target_permanent(e: &GameEngine, oid: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: oid,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(
            e.state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0),
        ),
        ..Default::default()
    }]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &["grizzly_bears", "swords_to_plowshares"]),
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

#[test]
fn tap_ping_self_batch_cards_deal_two_damage_to_any_target_and_three_to_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_320_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("activate at the opponent");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 18,
            "{}: two damage to the target player",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 17,
            "{}: three damage to the controller",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the tap cost was paid",
            case.id
        );
    }
}

#[test]
fn tap_ping_self_batch_cards_damage_their_controller_even_when_hitting_a_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_321_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 4);
        let target = target_permanent(&e, bear);
        apply_ability(&mut e, 0, source, 0, target).expect("activate at a creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&bear].damage, 2,
            "{}: two damage to the creature",
            case.id
        );
        assert!(
            e.state.players[1].battlefield.contains(&bear),
            "{}: the 2/4 survives two damage",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 17,
            "{}: three damage to the controller",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: the target's controller does not gain or lose life",
            case.id
        );
    }
}

#[test]
fn tap_ping_self_batch_cards_fizzle_without_self_damage_when_the_target_is_gone() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_322_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 4);
        let target = target_permanent(&e, bear);
        apply_ability(&mut e, 0, source, 0, target).expect("activate at the bear");
        e.apply_scenario_command(0, &pass())
            .expect("the activator passes priority");
        ensure_in_hand(&mut e, 1, "swords_to_plowshares");
        give_mana(
            &mut e,
            1,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let swords = hand_index_for_card(&e, 1, "swords_to_plowshares");
        let target = target_permanent(&e, bear);
        e.apply_scenario_command(1, &cast_spell(swords, target))
            .expect("exile the target in response");
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.objects[&bear].zone,
            Zone::Exile,
            "{}: the target left the battlefield",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: the fizzled ability does not damage its controller",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the activation cost remains paid",
            case.id
        );
    }
}

#[test]
fn tap_ping_self_batch_cards_require_an_untapped_source_and_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_323_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("first activation");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: a tapped source cannot pay the tap cost again",
            case.id
        );

        let mut tapped_e = engine(9_324_000 + index as u64, case.id);
        let tapped = move_ready_to_battlefield(&mut tapped_e, 0, case.id);
        tapped_e
            .state
            .objects
            .get_mut(&tapped)
            .expect("source")
            .tapped = true;
        tapped_e.initial_response_batch();
        assert!(
            apply_ability(&mut tapped_e, 0, tapped, 0, target_player(1)).is_err(),
            "{}: an already tapped source cannot activate",
            case.id
        );

        let mut foreign_e = engine(9_325_000 + index as u64, case.id);
        let foreign = move_ready_to_battlefield(&mut foreign_e, 0, case.id);
        assert!(
            apply_ability(&mut foreign_e, 1, foreign, 0, target_player(0)).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
    }
}
