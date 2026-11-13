//! Actual-card scenarios for the reviewed sacrifice-to-destroy-enchantment family.
//!
//! Expectations are printed Oracle values. CR 602.2b/601.2h pay the sacrifice as a cost during
//! activation and before resolution; CR 701.21a performs the sacrifice; CR 701.8a destroys the
//! chosen enchantment on resolution; CR 608.2b fizzles when that target already left.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

fn cases() -> [&'static str; 3] {
    ["kami_of_ancient_law", "keening_apparition", "ronom_unicorn"]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
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
fn ronom_unicorn_batch_cards_sacrifice_as_a_cost_and_destroy_only_enchantments() {
    for (index, id) in cases().into_iter().enumerate() {
        let mut e = engine(850_000 + index as u64, id);
        let source = inject_permanent_on_battlefield(&mut e, 0, id);
        let enchantment = inject_permanent_on_battlefield(&mut e, 1, "crusade");
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(creature)),
            )
            .is_err(),
            "{id}: only enchantments are legal targets"
        );
        assert_eq!(e.state.objects[&source].zone, Zone::Battlefield, "{id}");
        assert!(e.state.stack.is_empty(), "{id}");

        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(enchantment)),
        )
        .expect("sacrifice the creature and target the enchantment");
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{id}: the sacrifice is paid as a cost"
        );
        assert_eq!(
            e.state.objects[&enchantment].zone,
            Zone::Battlefield,
            "{id}"
        );
        assert_eq!(e.state.stack.len(), 1, "{id}");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard, "{id}");
        assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield, "{id}");
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(creature)),
            )
            .is_err(),
            "{id}: the sacrificed source cannot activate again"
        );
    }
}

#[test]
fn ronom_unicorn_batch_cards_fizzle_when_the_enchantment_already_left() {
    for (index, id) in cases().into_iter().enumerate() {
        let mut e = engine(851_000 + index as u64, id);
        let source = inject_permanent_on_battlefield(&mut e, 0, id);
        let enchantment = inject_permanent_on_battlefield(&mut e, 1, "crusade");
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(enchantment)),
        )
        .expect("activate targeting the enchantment");
        e.state.players[1]
            .battlefield
            .retain(|oid| *oid != enchantment);
        e.state
            .objects
            .get_mut(&enchantment)
            .expect("enchantment")
            .zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(enchantment);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{id}: the cost stays paid"
        );
        assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard, "{id}");
        assert!(e.state.stack.is_empty(), "{id}");
    }
}
