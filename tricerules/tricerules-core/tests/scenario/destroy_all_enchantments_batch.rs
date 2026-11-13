//! Actual-card scenarios for the reviewed "Destroy all enchantments" family.
//!
//! Expectations are printed Oracle values. CR 701.8 destroys every enchantment
//! on every battlefield; the effect is symmetric and never touches artifacts,
//! creatures, or lands. An empty board still resolves normally.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "back_to_nature",
        },
        Case { id: "cleanfall" },
        Case {
            id: "tempest_of_light",
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[card, "crusade", "grizzly_bears", "bonesplitter"],
        ),
        deck_with("forest", &["guardian_project", "grizzly_bears"]),
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

fn cast_named(e: &mut GameEngine, card: &str) {
    ensure_in_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error}"));
    resolve_entire_stack_two_player(e);
}

#[test]
fn destroy_all_enchantments_batch_cards_destroy_every_enchantment() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_110_000 + index as u64, case.id);
        let own = move_ready_to_battlefield(&mut e, 0, "crusade");
        let opposing = move_ready_to_battlefield(&mut e, 1, "guardian_project");
        let own_creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opposing_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_named(&mut e, case.id);
        assert_eq!(
            e.state.objects[&own].zone,
            Zone::Graveyard,
            "{}: a friendly enchantment is destroyed",
            case.id
        );
        assert_eq!(
            e.state.objects[&opposing].zone,
            Zone::Graveyard,
            "{}: an opposing enchantment is destroyed",
            case.id
        );
        assert!(
            e.state.players[0].battlefield.contains(&own_creature),
            "{}: creatures are unaffected",
            case.id
        );
        assert!(
            e.state.players[1].battlefield.contains(&opposing_creature),
            "{}",
            case.id
        );
    }
}

#[test]
fn destroy_all_enchantments_batch_cards_leave_nonenchantments() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_111_000 + index as u64, case.id);
        let enchantment = move_ready_to_battlefield(&mut e, 0, "crusade");
        let artifact = inject_permanent_on_battlefield(&mut e, 0, "bonesplitter");
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        cast_named(&mut e, case.id);
        assert_eq!(
            e.state.objects[&enchantment].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        for oid in [artifact, creature, land] {
            assert!(
                e.state.players[0].battlefield.contains(&oid),
                "{}: nonenchantment {oid} survives",
                case.id
            );
        }
    }
}

#[test]
fn destroy_all_enchantments_batch_cards_resolve_with_no_enchantments() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_112_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let life_before = e.state.players[0].life;
        cast_named(&mut e, case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(
            e.state.players[0].battlefield.contains(&creature),
            "{}: the board is unchanged",
            case.id
        );
        assert_eq!(e.state.players[0].life, life_before, "{}", case.id);
    }
}
