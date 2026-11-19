//! Actual-card scenarios for the queue-v42 keyword-plus-cycling creature cohort.
//!
//! Expectations are printed Oracle values. CR 702.29a-b make Cycling a hand-zone activated
//! ability paid with the printed mana and discarding this card, then drawing one card. The
//! printed keyword and protection statics are checked in derived characteristics (CR 702).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::{Color, Keyword, ProtectionQuality};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

struct Case {
    id: &'static str,
    cost: ManaGift,
    partial: ManaGift,
    body: (u32, u32),
    keyword: Option<Keyword>,
    protection: Option<Color>,
}

fn cases() -> [Case; 8] {
    [
        Case {
            id: "wasteland_scorpion",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            partial: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            keyword: Some(Keyword::Deathtouch),
            protection: None,
        },
        Case {
            id: "lava_serpent",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            partial: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: (5, 5),
            keyword: Some(Keyword::Haste),
            protection: None,
        },
        Case {
            id: "bloated_toad",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            partial: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            keyword: None,
            protection: Some(Color::Blue),
        },
        Case {
            id: "disciple_of_malice",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            partial: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: (1, 2),
            keyword: None,
            protection: Some(Color::White),
        },
        Case {
            id: "moaning_wall",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            partial: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: (0, 5),
            keyword: Some(Keyword::Defender),
            protection: None,
        },
        Case {
            id: "imposing_vantasaur",
            cost: ManaGift {
                c: 1,
                ..Default::default()
            },
            partial: ManaGift::default(),
            body: (3, 6),
            keyword: Some(Keyword::Vigilance),
            protection: None,
        },
        Case {
            id: "granitic_titan",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            partial: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: (5, 4),
            keyword: Some(Keyword::Menace),
            protection: None,
        },
        Case {
            id: "rampaging_hippo",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            partial: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: (5, 6),
            keyword: Some(Keyword::Trample),
            protection: None,
        },
    ]
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn hand_ability(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

fn cycle(e: &mut GameEngine, case: &Case) -> (u32, u32) {
    ensure_in_hand(e, 0, case.id);
    let card = e.state.players[0].hand[hand_index_for_card(e, 0, case.id)];
    let drawn = put_on_top(e, 0, "grizzly_bears");
    give_mana(e, 0, gift(&case.cost));
    e.apply_scenario_command(0, &hand_ability(e, card))
        .expect("activate cycling from the hand");
    (card, drawn)
}

#[test]
fn q42_cycling_keyword_canary_simplest_cycles_for_two_and_draws_one() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "wasteland_scorpion")
        .expect("Wasteland Scorpion case");
    let mut e = engine(42_010_001, case.id);
    let (card, drawn) = cycle(&mut e, &case);
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Graveyard,
        "cycling discards itself as a cost"
    );
    let hand_before = e.state.players[0].hand.len();
    let library_before = e.state.players[0].library.len();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "cycling draws exactly one card"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "the draw comes from the controller's library"
    );
    assert!(e.state.players[0].hand.contains(&drawn));
    assert_eq!(
        pool(&e, 0),
        (0, 0, 0, 0, 0, 0),
        "the exact cost is consumed"
    );
}

#[test]
fn q42_cycling_keyword_canary_hardest_uses_the_printed_cycle_cost() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "imposing_vantasaur")
        .expect("Imposing Vantasaur case");
    let mut e = engine(42_010_002, case.id);
    ensure_in_hand(&mut e, 0, case.id);
    let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
    give_mana(&mut e, 0, case.partial);
    assert!(
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .is_err(),
        "the printed {{1}} cycling cost is required"
    );
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Hand,
        "a rejected activation leaves the card in hand"
    );
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
    give_mana(&mut e, 0, case.cost);
    e.apply_scenario_command(0, &hand_ability(&e, card))
        .expect("activate the printed cycling cost");
    assert_eq!(e.state.objects[&card].zone, Zone::Graveyard);
}

#[test]
fn q42_cycling_keyword_cards_cycle_for_their_printed_cost_and_draw_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_011_000 + index as u64, case.id);
        let (card, drawn) = cycle(&mut e, &case);
        assert_eq!(
            e.state.objects[&card].zone,
            Zone::Graveyard,
            "{}: cycling discards itself as a cost",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: the printed cycling cost is exact",
            case.id
        );
        let hand_before = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.clone();
        let opponent_library = e.state.players[1].library.clone();
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: cycling draws one card",
            case.id
        );
        assert_eq!(
            e.state.objects[&drawn].zone,
            Zone::Hand,
            "{}: the drawn card is in hand",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand, opponent_hand,
            "{}: the opponent does not draw",
            case.id
        );
        assert_eq!(
            e.state.players[1].library, opponent_library,
            "{}: the opponent's library is untouched",
            case.id
        );
    }
}

#[test]
fn q42_cycling_keyword_cards_publish_their_printed_static() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_012_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        match case.keyword {
            Some(keyword) => assert!(
                e.effective_has_keyword(source, keyword),
                "{}: printed keyword",
                case.id
            ),
            None => assert!(
                e.characteristics(source)
                    .expect("characteristics")
                    .keywords
                    .is_empty(),
                "{}: no printed keyword",
                case.id
            ),
        }
        match case.protection {
            Some(color) => assert_eq!(
                e.characteristics(source)
                    .expect("characteristics")
                    .protections,
                [ProtectionQuality::Color(color)],
                "{}",
                case.id
            ),
            None => assert!(
                e.characteristics(source)
                    .expect("characteristics")
                    .protections
                    .is_empty(),
                "{}",
                case.id
            ),
        }
    }
}

#[test]
fn q42_cycling_keyword_cards_reject_activation_outside_the_hand() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_013_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.cost));
        assert!(
            e.apply_scenario_command(0, &hand_ability(&e, source))
                .is_err(),
            "{}: cycling functions only from the hand",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: the permanent stays on the battlefield",
            case.id
        );
        let cost = case.cost;
        assert_eq!(
            pool(&e, 0),
            (cost.w, cost.u, cost.b, cost.r, cost.g, cost.c),
            "{}: a rejected activation spends no mana",
            case.id
        );
    }
}
