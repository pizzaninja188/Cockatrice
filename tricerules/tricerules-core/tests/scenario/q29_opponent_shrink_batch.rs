//! Queue-v29 canaries: "Creatures your opponents control get -1/-0."
//!
//! CR 604.2/611.3b and 613.1/613.4c. Expectations are printed Oracle values.
use super::helpers::*;

struct Case {
    id: &'static str,
    own_power: u32,
    own_toughness: u32,
}

fn case(id: &'static str) -> Case {
    match id {
        "cumber_stone" => Case {
            id,
            own_power: 2,
            own_toughness: 2,
        },
        "haunter_of_nightveil" => Case {
            id,
            own_power: 3,
            own_toughness: 4,
        },
        "dampening_pulse" => Case {
            id,
            own_power: 2,
            own_toughness: 2,
        },
        _ => unreachable!(),
    }
}

fn cases() -> [Case; 3] {
    [
        case("cumber_stone"),
        case("dampening_pulse"),
        case("haunter_of_nightveil"),
    ]
}

fn mana_for(id: &str) -> ManaGift {
    match id {
        "cumber_stone" | "dampening_pulse" => ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        "haunter_of_nightveil" => ManaGift {
            u: 1,
            b: 1,
            c: 3,
            ..Default::default()
        },
        _ => unreachable!(),
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears"]),
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

fn assert_canary(case: Case, seed: u64) {
    let mut e = engine(seed, case.id);
    if case.id == "cumber_stone" {
        move_ready_to_battlefield(&mut e, 0, case.id);
    } else {
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.own_power), Some(case.own_toughness)),
            "{}: the source does not modify itself",
            case.id
        );
    }
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert_eq!(
        (e.effective_power(opponent), e.effective_toughness(opponent)),
        (Some(1), Some(2)),
        "{}: opponents' creatures lose one power only",
        case.id
    );
    assert_eq!(
        (e.effective_power(own), e.effective_toughness(own)),
        (Some(2), Some(2)),
        "{}: the controller's own creatures are unaffected",
        case.id
    );
}

#[test]
fn q29_opponent_shrink_canary_simplest() {
    assert_canary(case("cumber_stone"), 29_070_001);
}

#[test]
fn q29_opponent_shrink_canary_hardest() {
    assert_canary(case("haunter_of_nightveil"), 29_070_002);
}

#[test]
fn q29_opponent_shrink_cards_cast_with_exact_cost_and_apply_after_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_071_000 + index as u64, case.id);
        give_mana(&mut e, 0, mana_for(case.id));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        if case.id == "haunter_of_nightveil" {
            let source = battlefield_object_for_card(&e, 0, case.id);
            assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                (Some(case.own_power), Some(case.own_toughness)),
                "{}: the source's own body",
                case.id
            );
        }
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        assert_eq!(
            (e.effective_power(opponent), e.effective_toughness(opponent)),
            (Some(1), Some(2)),
            "{}: the static effect covers later entrants",
            case.id
        );
    }
}

#[test]
fn q29_opponent_shrink_cards_apply_to_every_opponent_in_a_three_player_game() {
    let decks = Some(vec![
        deck_with("island", &["cumber_stone"]),
        deck_with("forest", &["grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        29_072_000,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    move_ready_to_battlefield(&mut e, 0, "cumber_stone");
    let first = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 2, "grizzly_bears");
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert_eq!(e.effective_power(first), Some(1));
    assert_eq!(e.effective_power(second), Some(1));
    assert_eq!(e.effective_power(own), Some(2));
}

#[test]
fn q29_opponent_shrink_cards_stack_with_each_other() {
    let decks = Some(vec![
        deck_with("island", &["cumber_stone", "dampening_pulse"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        29_073_000,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    move_ready_to_battlefield(&mut e, 0, "cumber_stone");
    move_ready_to_battlefield(&mut e, 0, "dampening_pulse");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    assert_eq!(
        (e.effective_power(opponent), e.effective_toughness(opponent)),
        (Some(0), Some(2)),
        "two -1/-0 effects stack"
    );
}
