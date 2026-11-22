//! Actual-card scenarios for the reviewed "{T}: This creature deals 1 damage to
//! any target" family.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activate the ability,
//! CR 118.3 requires the tap cost to be fully payable, and CR 608.2b leaves a
//! stale target without damage. The source must be untapped and controlled by
//! the activating player.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "razorfin_hunter",
        },
        Case {
            id: "rootwater_hunter",
        },
        Case {
            id: "viashino_fangtail",
        },
        Case {
            id: "zuran_spellcaster",
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
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
fn pinger_batch_cards_deal_one_damage_to_any_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_130_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("activate at the opponent");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 19,
            "{}: one damage to the opponent",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the tap cost was paid",
            case.id
        );

        let mut creature_e = engine(9_131_000 + index as u64, case.id);
        let creature_source = move_ready_to_battlefield(&mut creature_e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut creature_e, 1, "grizzly_bears");
        apply_ability(&mut creature_e, 0, creature_source, 0, target_object(bear))
            .expect("activate at a creature");
        resolve_entire_stack_two_player(&mut creature_e);
        assert_eq!(
            creature_e.state.objects[&bear].damage, 1,
            "{}: one damage to the creature",
            case.id
        );
        assert!(
            creature_e.state.players[1].battlefield.contains(&bear),
            "{}: the 2/2 survives one damage",
            case.id
        );
    }
}

#[test]
fn pinger_batch_cards_stop_when_the_target_is_gone() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_132_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        apply_ability(&mut e, 0, source, 0, target_object(bear)).expect("activate at the bear");
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
        e.apply_scenario_command(1, &cast_spell(swords, target_object(bear)))
            .expect("exile the target in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&bear].zone,
            Zone::Exile,
            "{}: the target left the battlefield",
            case.id
        );
        assert_eq!(
            e.state.objects[&bear].damage, 0,
            "{}: the stale ability deals no damage",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the source remains tapped",
            case.id
        );
    }
}

#[test]
fn pinger_batch_cards_require_tap_and_untapped_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_133_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("first activation");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: a tapped source cannot pay the tap cost again",
            case.id
        );

        let mut tapped_e = engine(9_134_000 + index as u64, case.id);
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

        let mut foreign_e = engine(9_135_000 + index as u64, case.id);
        let foreign = move_ready_to_battlefield(&mut foreign_e, 0, case.id);
        assert!(
            apply_ability(&mut foreign_e, 1, foreign, 0, target_player(0)).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
    }
}
