//! Queue-v29 canaries: "Destroy all Islands."
//!
//! CR 701.8, 305.6, 608.2 and 701.8c. Expectations are printed Oracle values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
}

fn case(id: &'static str) -> Case {
    match id {
        "boiling_seas" | "boil" | "tsunami" => Case { id },
        _ => unreachable!(),
    }
}

fn mana_for(id: &str) -> ManaGift {
    match id {
        "boiling_seas" | "boil" => ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
        "tsunami" => ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
        _ => unreachable!(),
    }
}

fn cases() -> [Case; 3] {
    [case("boiling_seas"), case("boil"), case("tsunami")]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn assert_canary(case: Case, seed: u64) {
    let mut e = engine(seed, case.id);
    let own_island = inject_permanent_on_battlefield(&mut e, 0, "island");
    let other_island = inject_permanent_on_battlefield(&mut e, 1, "island");
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let vivid_creek = inject_permanent_on_battlefield(&mut e, 1, "vivid_creek");
    give_mana(&mut e, 0, mana_for(case.id));
    inject_card_into_hand(&mut e, 0, case.id);
    let hand = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(&mut e);
    for island in [own_island, other_island] {
        assert_eq!(
            e.state.objects[&island].zone,
            Zone::Graveyard,
            "{}: an Island is destroyed",
            case.id
        );
    }
    assert_eq!(
        e.state.objects[&forest].zone,
        Zone::Battlefield,
        "{}: a Forest is not an Island",
        case.id
    );
    assert_eq!(
        e.state.objects[&vivid_creek].zone,
        Zone::Battlefield,
        "{}: a non-Island land survives",
        case.id
    );
    assert!(e.state.stack.is_empty(), "{}", case.id);
}

#[test]
fn q29_destroy_islands_canary_simplest() {
    assert_canary(case("boiling_seas"), 29_080_001);
}

#[test]
fn q29_destroy_islands_canary_hardest() {
    assert_canary(case("boil"), 29_080_002);
}

#[test]
fn q29_destroy_islands_cards_destroy_every_island_including_snow_islands() {
    for (index, case) in cases().into_iter().enumerate() {
        let id = case.id;
        assert_canary(case, 29_081_000 + index as u64);
        let mut e = engine(29_082_000 + index as u64, id);
        let snow_island = inject_permanent_on_battlefield(&mut e, 0, "snow-covered_island");
        let island = inject_permanent_on_battlefield(&mut e, 1, "island");
        give_mana(&mut e, 0, mana_for(id));
        inject_card_into_hand(&mut e, 0, id);
        let hand = hand_index_for_card(&e, 0, id);
        e.apply_scenario_command(0, &cast_spell(hand, vec![]))
            .expect("cast the mass destruction spell");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&snow_island].zone, Zone::Graveyard);
        assert_eq!(e.state.objects[&island].zone, Zone::Graveyard);
    }
}

#[test]
fn q29_destroy_islands_boil_is_an_instant_and_the_sorceries_are_not() {
    let mut instant = engine(29_083_000, "boil");
    inject_card_into_hand(&mut instant, 0, "grizzly_bears");
    give_mana(
        &mut instant,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bears = hand_index_for_card(&instant, 0, "grizzly_bears");
    instant
        .apply_scenario_command(0, &cast_spell(bears, vec![]))
        .expect("cast a creature spell");
    give_mana(
        &mut instant,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut instant, 0, "boil");
    let boil = hand_index_for_card(&instant, 0, "boil");
    instant
        .apply_scenario_command(0, &cast_spell(boil, vec![]))
        .expect("cast the instant in response");
    assert_eq!(instant.state.stack.len(), 2);
    resolve_entire_stack_two_player(&mut instant);
    assert!(instant.state.stack.is_empty());

    for id in ["boiling_seas", "tsunami"] {
        let mut e = engine(29_084_000, id);
        inject_card_into_hand(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        let bears = hand_index_for_card(&e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &cast_spell(bears, vec![]))
            .expect("cast a creature spell");
        give_mana(&mut e, 0, mana_for(id));
        inject_card_into_hand(&mut e, 0, id);
        let hand = hand_index_for_card(&e, 0, id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand, vec![]))
                .is_err(),
            "{id}: a sorcery cannot be cast while the stack is nonempty"
        );
    }
}
