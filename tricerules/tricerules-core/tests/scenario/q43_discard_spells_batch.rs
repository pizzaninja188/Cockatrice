//! Actual-card scenarios for the queue-v43 target-player discard cohort.
//!
//! Expectations are printed Oracle values. CR 601.2a casts the spell; CR 608.2 resolves it;
//! CR 115.1 targets one player and CR 701.8a makes that player discard the printed number of
//! cards. The affected player chooses which cards leave a private hand.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    count: usize,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "fugue",
            mana: ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            count: 3,
        },
        Case {
            id: "three_tragedies",
            mana: ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            count: 3,
        },
        Case {
            id: "waking_nightmare",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            count: 2,
        },
    ]
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[card]), deck_with("forest", &[])]);
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
    let hand = std::mem::take(&mut e.state.players[0].hand);
    e.state.players[0].library.extend(hand);
    let hand = std::mem::take(&mut e.state.players[1].hand);
    e.state.players[1].library.extend(hand);
    e
}

fn clear_opponent_hand(e: &mut GameEngine) {
    let hand = std::mem::take(&mut e.state.players[1].hand);
    e.state.players[1].library.extend(hand);
}

fn cast_discard(e: &mut GameEngine, case: &Case) -> RuledEventBatch {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    let batch = pass_both_players_with_batch(e);
    let p = &e.state.players[0].mana_pool;
    assert_eq!(
        (p.white, p.blue, p.black, p.red, p.green, p.colorless),
        (0, 0, 0, 0, 0, 0),
        "{}: exact cost",
        case.id
    );
    batch
}

#[test]
fn q43_discard_spells_canary_discards_the_printed_count() {
    let case = &cases()[0];
    let mut e = engine(43_070_001, case.id);
    clear_opponent_hand(&mut e);
    let controlled: Vec<u32> = ["plains", "island", "swamp", "mountain", "forest"]
        .into_iter()
        .map(|card| inject_card_into_hand(&mut e, 1, card))
        .collect();
    let batch = cast_discard(&mut e, case);
    let offer =
        find_resolution_choice(&batch).expect("the affected player chooses from a private hand");
    assert!(
        offer.public_reveal.is_none(),
        "the discard choice does not reveal the hand"
    );
    e.apply_scenario_command(1, &submit_resolution_choice(controlled[..3].to_vec()))
        .expect("the affected player chooses the three cards");
    let discarded = controlled[..3]
        .iter()
        .filter(|oid| e.state.objects[oid].zone == Zone::Graveyard)
        .count();
    assert_eq!(
        discarded, 3,
        "exactly three chosen cards reach the graveyard"
    );
    assert_eq!(e.state.players[1].hand.len(), 2, "the rest stay in hand");
}

#[test]
fn q43_discard_spells_cards_discard_their_printed_count_from_the_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_071_000 + index as u64, case.id);
        clear_opponent_hand(&mut e);
        let controlled: Vec<u32> = ["plains", "island", "swamp", "mountain", "forest"]
            .into_iter()
            .map(|card| inject_card_into_hand(&mut e, 1, card))
            .collect();
        cast_discard(&mut e, case);
        e.apply_scenario_command(
            1,
            &submit_resolution_choice(controlled[..case.count].to_vec()),
        )
        .unwrap_or_else(|error| panic!("{}: choose discard: {error:?}", case.id));
        for oid in &controlled[..case.count] {
            assert_eq!(
                e.state.objects[oid].zone,
                Zone::Graveyard,
                "{}: selected card is discarded",
                case.id
            );
        }
        assert_eq!(
            e.state.players[1].hand.len(),
            5 - case.count,
            "{}: exactly the printed count leaves the hand",
            case.id
        );
        assert!(
            e.state.players[0].hand.is_empty(),
            "{}: the caster discards nothing and the spell has left hand",
            case.id
        );
    }
}

#[test]
fn q43_discard_spells_cards_require_mana_and_empty_a_short_hand() {
    let mut e = engine(43_072_001, "fugue");
    clear_opponent_hand(&mut e);
    let card = inject_card_into_hand(&mut e, 0, "fugue");
    let slot = hand_index_for_card(&e, 0, "fugue");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .is_err(),
        "the spell requires its printed mana"
    );
    assert!(e.state.players[0].hand.contains(&card), "the card stays");
    assert!(e.state.players[1].hand.is_empty());

    let mut short = engine(43_073_001, "fugue");
    clear_opponent_hand(&mut short);
    let only = [
        inject_card_into_hand(&mut short, 1, "plains"),
        inject_card_into_hand(&mut short, 1, "island"),
    ];
    cast_discard(&mut short, &cases()[0]);
    short
        .apply_scenario_command(1, &submit_resolution_choice(only.to_vec()))
        .expect("a short hand discards all eligible cards");
    assert_eq!(
        short.state.players[1].hand.len(),
        0,
        "the short hand empties"
    );
}

#[test]
fn q43_discard_spells_cards_may_target_the_caster() {
    let mut e = engine(43_074_001, "waking_nightmare");
    let controlled: Vec<u32> = ["plains", "island", "swamp"]
        .into_iter()
        .map(|card| inject_card_into_hand(&mut e, 0, card))
        .collect();
    inject_card_into_hand(&mut e, 0, "waking_nightmare");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "waking_nightmare");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(0)))
        .expect("the caster may target itself");
    pass_both_players(&mut e);
    e.apply_scenario_command(0, &submit_resolution_choice(controlled[..2].to_vec()))
        .expect("the caster discards two chosen cards");
    assert_eq!(
        e.state.objects[&controlled[0]].zone,
        Zone::Graveyard,
        "the first chosen card is discarded"
    );
    assert_eq!(
        e.state.objects[&controlled[1]].zone,
        Zone::Graveyard,
        "the second chosen card is discarded"
    );
    assert_eq!(
        e.state.objects[&controlled[2]].zone,
        Zone::Hand,
        "the unchosen card stays in hand"
    );
}
