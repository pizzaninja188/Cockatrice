//! Actual-card scenarios for the reviewed sacrifice-for-three-mana attendant cohort.
//!
//! Expectations are printed Oracle values. CR 605.1a and 605.3a-b make the targetless mana
//! ability resolve immediately even though it sacrifices the source; CR 118.3 and
//! 701.21a govern the sacrifice cost; CR 106.1a-b produce one mana per symbol.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    produced: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "crosiss_attendant",
            produced: (0, 1, 1, 1, 0, 0),
        },
        Case {
            id: "darigaazs_attendant",
            produced: (0, 0, 1, 1, 1, 0),
        },
        Case {
            id: "dromars_attendant",
            produced: (1, 1, 1, 0, 0, 0),
        },
        Case {
            id: "riths_attendant",
            produced: (1, 0, 0, 1, 1, 0),
        },
        Case {
            id: "trevas_attendant",
            produced: (1, 1, 0, 0, 1, 0),
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "trevas_attendant" | "crosiss_attendant"))
        .collect()
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(
        e,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    inject_card_into_hand(e, 0, case.id);
    let hand = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    let source = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(3), Some(3)),
        "{}: printed 3/3 body",
        case.id
    );
    source
}

fn activate_sacrifice(e: &mut GameEngine, source: u32, id: &str) {
    give_mana(
        e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(e, source, 0, vec![]))
        .unwrap_or_else(|error| panic!("{id}: activate the sacrifice mana ability: {error:?}"));
}

#[test]
fn sac_creature_three_batch_canary_cards_cast_with_the_printed_body() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(890_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(
            !e.state.objects[&source].tapped,
            "{}: enters untapped",
            case.id
        );
    }
}

#[test]
fn sac_creature_three_batch_canary_cards_sacrifice_for_the_printed_mana() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(891_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        activate_sacrifice(&mut e, source, case.id);
        assert_eq!(pool(&e), case.produced, "{}: printed mana", case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: the creature is sacrificed as a cost",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: mana abilities do not use the stack",
            case.id
        );
    }
}

#[test]
fn sac_creature_three_batch_cards_sacrifice_for_the_printed_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(892_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        activate_sacrifice(&mut e, source, case.id);
        assert_eq!(pool(&e), case.produced, "{}: printed mana", case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: sacrificed",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn sac_creature_three_batch_cards_reject_the_activation_without_one_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(893_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: the activation needs one generic mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: a rejected activation does not sacrifice the source",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn sac_creature_three_batch_cards_cannot_reactivate_a_sacrificed_attendant() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(894_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        activate_sacrifice(&mut e, source, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let p = &e.state.players[0].mana_pool;
        let before = (p.white, p.blue, p.black, p.red, p.green, p.colorless);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: a sacrificed permanent cannot be activated again",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: the rejected activation spends nothing",
            case.id
        );
    }
}
