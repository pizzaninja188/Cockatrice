//! Actual-card scenarios for the queue-v80 01-sliver-pump-grant cohort.
//!
//! Expectations are printed Oracle values. CR 113.10 and 602.2 make the quoted activated ability
//! an ability of every Sliver creature, including the source and opponents' Slivers; CR 611.3a and
//! 613.4c make the pump cumulative. The pinned 2013-07-01 rulings confirm cumulative grants and
//! that a creature that stops being a Sliver no longer receives its own grant.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::AbilityInfo;

struct Case {
    id: &'static str,
    card_id: &'static str,
    power: i32,
    toughness: i32,
}

const CASES: [Case; 3] = [
    Case {
        id: "armor_sliver",
        card_id: "armor_sliver",
        power: 0,
        toughness: 1,
    },
    Case {
        id: "barbed_sliver",
        card_id: "barbed_sliver",
        power: 1,
        toughness: 0,
    },
    Case {
        id: "spectral_sliver",
        card_id: "spectral_sliver",
        power: 1,
        toughness: 1,
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "armor_sliver",
                "barbed_sliver",
                "spectral_sliver",
                "metallic_sliver",
                "grizzly_bears",
            ],
        ),
        deck_with("island", &["metallic_sliver"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn catalog(engine: &mut GameEngine, source: u32) -> Vec<AbilityInfo> {
    engine
        .initial_response_batch()
        .events
        .into_iter()
        .find_map(|event| match event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .into_iter()
                .flat_map(|player| player.battlefield_objects)
                .find(|object| object.object_id == source)
                .map(|object| object.activated_abilities),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no public battlefield catalog for {source}"))
}

fn granted_slot(engine: &mut GameEngine, source: u32, card_id: &str) -> u32 {
    catalog(engine, source)
        .into_iter()
        .find(|info| {
            info.presentation
                .as_ref()
                .is_some_and(|presentation| presentation.card_id == card_id)
        })
        .unwrap_or_else(|| {
            let available = catalog(engine, source)
                .into_iter()
                .filter_map(|info| info.presentation.map(|presentation| presentation.card_id))
                .collect::<Vec<_>>();
            panic!("{card_id} grants an ability to {source}; available: {available:?}")
        })
        .ability_index
}

fn pump(engine: &mut GameEngine, actor: i32, recipient: u32, card_id: &str) {
    give_mana(
        engine,
        actor,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = granted_slot(engine, recipient, card_id);
    engine
        .apply_scenario_command(
            actor,
            &activate_ability_for(engine, recipient, slot, vec![]),
        )
        .unwrap_or_else(|error| panic!("activate {card_id} grant on {recipient}: {error:?}"));
    resolve_entire_stack_two_player(engine);
}

#[test]
fn q80_sliver_pump_canary_simplest_armor_sliver_grants_toughness() {
    let mut e = engine(80_100_001);
    let armor = move_ready_to_battlefield(&mut e, 0, "armor_sliver");
    assert_eq!(e.effective_power(armor), Some(2));
    assert_eq!(e.effective_toughness(armor), Some(2));
    pump(&mut e, 0, armor, "armor_sliver");
    assert_eq!(
        e.effective_power(armor),
        Some(2),
        "Armor Sliver adds no power"
    );
    assert_eq!(
        e.effective_toughness(armor),
        Some(3),
        "Armor Sliver adds one toughness"
    );
    assert_eq!(e.state.objects[&armor].zone, Zone::Battlefield);
}

#[test]
fn q80_sliver_pump_canary_hardest_barbed_sliver_pumps_each_sliver_and_is_cumulative() {
    let mut e = engine(80_100_002);
    let barbed = move_ready_to_battlefield(&mut e, 0, "barbed_sliver");
    let metallic = move_ready_to_battlefield(&mut e, 0, "metallic_sliver");

    pump(&mut e, 0, metallic, "barbed_sliver");
    assert_eq!(
        e.effective_power(metallic),
        Some(2),
        "another Sliver receives +1/+0"
    );
    assert_eq!(e.effective_toughness(metallic), Some(1));

    pump(&mut e, 0, barbed, "barbed_sliver");
    assert_eq!(
        e.effective_power(barbed),
        Some(3),
        "the source also receives its grant"
    );

    pump(&mut e, 0, barbed, "barbed_sliver");
    assert_eq!(
        e.effective_power(barbed),
        Some(4),
        "two activations are cumulative"
    );
    assert_eq!(e.effective_toughness(barbed), Some(2));
}

#[test]
fn q80_sliver_pump_cards_all_apply_their_printed_pump() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(80_101_000 + index as u64);
        let sliver = move_ready_to_battlefield(&mut e, 0, case.id);
        pump(&mut e, 0, sliver, case.card_id);
        assert_eq!(
            e.effective_power(sliver),
            Some((2 + case.power) as u32),
            "{} power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(sliver),
            Some((2 + case.toughness) as u32),
            "{} toughness",
            case.id
        );
    }
}

#[test]
fn q80_sliver_pump_scope_includes_each_players_slivers_but_not_other_creatures() {
    let mut e = engine(80_102_001);
    let armor = move_ready_to_battlefield(&mut e, 0, "armor_sliver");
    let opposing = move_ready_to_battlefield(&mut e, 1, "metallic_sliver");
    let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");

    assert!(
        catalog(&mut e, opposing).iter().any(|info| info
            .presentation
            .as_ref()
            .is_some_and(|presentation| presentation.card_id == "armor_sliver")),
        "an opponent's Sliver receives the grant"
    );
    assert!(
        !catalog(&mut e, bears).iter().any(|info| info
            .presentation
            .as_ref()
            .is_some_and(|presentation| presentation.card_id == "armor_sliver")),
        "a non-Sliver does not receive the grant"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = granted_slot(&mut e, armor, "armor_sliver");
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, bears, slot, vec![]))
            .is_err(),
        "the non-Sliver's reused slot is not a legal activation"
    );
    assert_eq!(e.state.objects[&bears].zone, Zone::Battlefield);
}

#[test]
fn q80_sliver_pump_grant_disappears_when_its_source_leaves() {
    let mut e = engine(80_103_001);
    let armor = move_ready_to_battlefield(&mut e, 0, "armor_sliver");
    let metallic = move_ready_to_battlefield(&mut e, 0, "metallic_sliver");
    let slot = granted_slot(&mut e, metallic, "armor_sliver");

    e.state.players[0].battlefield.retain(|id| *id != armor);
    e.state.players[0].graveyard.push(armor);
    e.state.objects.get_mut(&armor).unwrap().zone = Zone::Graveyard;

    assert!(
        !catalog(&mut e, metallic).iter().any(|info| info
            .presentation
            .as_ref()
            .is_some_and(|presentation| presentation.card_id == "armor_sliver")),
        "the departed source no longer grants an ability"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, metallic, slot, vec![]))
            .is_err(),
        "the stale granted slot is rejected"
    );
    assert_eq!(e.effective_toughness(metallic), Some(1));
}
