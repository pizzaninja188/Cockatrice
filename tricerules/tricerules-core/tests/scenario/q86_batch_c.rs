//! Actual-card scenarios for the queue-v86 03-x-entry-counters cohort.
//!
//! Five cards whose only non-keyword clause is "This creature enters with X +1/+1 counters on it":
//! Ivy Elemental ({X}{G}, 0/0), Nimbus Swimmer ({X}{G}{U}, 0/0, Flying), Shifting Wall ({X}, 0/0,
//! Defender), Shivan Devastator ({X}{R}, 0/0, Flying + haste) and Worldsoul Colossus ({X}{G}{W},
//! 0/0, Convoke).
//!
//! Expectations are printed Oracle values. CR 107.3a-b (X is chosen as the spell is cast), 601.2f
//! (the chosen X sets the total cost) and 614.1c/122.6 (the entry replacement adds the counters)
//! govern. CR 702.9 (flying), 702.10 (haste), 702.3 (defender) and 702.51 (convoke) cover the
//! printed keywords.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    /// The printed colored pips, supplied as their real colors.
    mana: ManaGift,
    /// How many colored pips the card prints; the all-colorless rejection test needs the exact
    /// generic total so that it fails only on the missing color.
    pips: u32,
    keywords: &'static [Keyword],
    convoke: bool,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "ivy_elemental",
            pips: 1,
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            keywords: &[],
            convoke: false,
        },
        Case {
            id: "nimbus_swimmer",
            pips: 2,
            mana: ManaGift {
                g: 1,
                u: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Flying],
            convoke: false,
        },
        Case {
            id: "shifting_wall",
            pips: 0,
            mana: ManaGift::default(),
            keywords: &[Keyword::Defender],
            convoke: false,
        },
        Case {
            id: "shivan_devastator",
            pips: 1,
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Flying, Keyword::Haste],
            convoke: false,
        },
        Case {
            id: "worldsoul_colossus",
            pips: 2,
            mana: ManaGift {
                g: 1,
                w: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Convoke],
            convoke: true,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("island", &["grizzly_bears"]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn plus_counters(e: &GameEngine, object: u32) -> u32 {
    e.state.objects[&object].counter_count(CounterKind::PlusOnePlusOne)
}

fn body(e: &GameEngine, object: u32) -> (Option<u32>, Option<u32>) {
    (e.effective_power(object), e.effective_toughness(object))
}

/// Cast the card for the given X and return the identity of the cast card, which stays stable
/// across the zone change so a 0/0 that dies immediately is still observable.
fn cast_for_x(e: &mut GameEngine, case: &Case, x: u32) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    give_mana(
        e,
        0,
        ManaGift {
            c: x,
            ..Default::default()
        },
    );
    let card = inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell_x(slot, vec![], x))
        .unwrap_or_else(|error| panic!("{}: cast with X={x}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost for X={x}",
        case.id
    );
    resolve_entire_stack_two_player(e);
    card
}

/// Ivy Elemental is the simplest canary: one colored pip, no printed keyword.
#[test]
fn q86_x_canary_ivy_elemental_enters_with_exactly_x_counters() {
    let case = &cases()[0];
    assert_eq!(case.id, "ivy_elemental");
    let mut e = engine(20_880_100);
    let source = cast_for_x(&mut e, case, 3);
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(plus_counters(&e, source), 3, "X = 3 counters");
    assert_eq!(
        body(&e, source),
        (Some(3), Some(3)),
        "printed 0/0 plus three counters"
    );
}

/// Worldsoul Colossus is the hardest canary: Convoke changes how the total cost is paid, but the
/// counters still equal the chosen X (CR 601.2f + 702.51).
#[test]
fn q86_x_canary_worldsoul_colossus_convoke_keeps_the_chosen_x() {
    let case = &cases()[4];
    assert_eq!(case.id, "worldsoul_colossus");
    assert!(case.convoke);
    let mut e = engine(20_880_101);
    // Pay the {G}{W} colored pips with mana and the generic portion with three convoked creatures.
    let payers = [
        inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2),
        inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3),
        inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2),
    ];
    give_mana(&mut e, 0, gift(&case.mana));
    // {X}{G}{W} with X=5 leaves {2} after three convoked creatures pay {3}.
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let card = inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    let object_ref = |object_id: u32| tricerules_proto::ruled::v1::CostObjectRef {
        object_id,
        zone_change_generation: e
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    };
    let command = tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(tricerules_proto::ruled::v1::ruled_command::Cmd::CastSpell(
            tricerules_proto::ruled::v1::CastSpell {
                cast_method: tricerules_proto::ruled::v1::CastMethod::Normal as i32,
                source: Some(hand_cast_source(slot)),
                x_value: 5,
                payment: Some(tricerules_proto::ruled::v1::PaymentSelection {
                    source: Some(object_ref(card)),
                    expected_state_revision: e.state.command_index,
                    mana: Some(tricerules_proto::ruled::v1::PaymentMana {
                        w: 1,
                        g: 1,
                        c: 2,
                        ..Default::default()
                    }),
                    convoke: payers
                        .iter()
                        .map(
                            |object_id| tricerules_proto::ruled::v1::ObjectPaymentContribution {
                                object: Some(object_ref(*object_id)),
                                kind: tricerules_proto::ruled::v1::ObjectPaymentKind::Generic
                                    as i32,
                            },
                        )
                        .collect(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
    };
    e.apply_scenario_command(0, &command)
        .expect("cast with convoke");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        plus_counters(&e, card),
        5,
        "the counters equal the chosen X, not the three tapped creatures"
    );
    assert_eq!(
        body(&e, card),
        (Some(5), Some(5)),
        "printed 0/0 plus five counters"
    );
    assert!(
        payers.iter().all(|object| e.state.objects[object].tapped),
        "every convoked creature is tapped"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the whole cost was paid");
    // The replacement is self-scoped: the convoked creatures never receive counters.
    for payer in payers {
        assert_eq!(
            e.state.objects[&payer].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "the entry replacement is self-scoped"
        );
    }
}

#[test]
fn q86_x_cards_enter_with_exactly_the_chosen_x() {
    for (index, case) in cases().iter().enumerate() {
        for x in [1u32, 4] {
            let mut e = engine(20_881_000 + index as u64 * 10 + x as u64);
            let source = cast_for_x(&mut e, case, x);
            assert_eq!(
                e.state.objects[&source].zone,
                Zone::Battlefield,
                "{}: a positive X keeps the 0/0 alive",
                case.id
            );
            assert_eq!(
                plus_counters(&e, source),
                x,
                "{}: X = {x} counters",
                case.id
            );
            assert_eq!(
                body(&e, source),
                (Some(x), Some(x)),
                "{}: printed 0/0 plus {x} counters",
                case.id
            );
        }
    }
}

#[test]
fn q86_x_cards_keep_their_printed_keywords() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_882_000 + index as u64);
        let source = cast_for_x(&mut e, case, 2);
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: printed {keyword:?}",
                case.id
            );
        }
        assert_eq!(
            e.effective_has_keyword(source, Keyword::Flying),
            case.keywords.contains(&Keyword::Flying),
            "{}: only the printed Flying",
            case.id
        );
        assert_eq!(
            e.effective_has_keyword(source, Keyword::Haste),
            case.keywords.contains(&Keyword::Haste),
            "{}: only the printed Haste",
            case.id
        );
        for absent in [Keyword::Defender, Keyword::Convoke] {
            assert_eq!(
                e.effective_has_keyword(source, absent),
                case.keywords.contains(&absent),
                "{}: only the printed {absent:?}",
                case.id
            );
        }
    }
}

#[test]
fn q86_x_cards_die_at_zero_toughness_when_cast_for_zero() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_883_000 + index as u64);
        let source = cast_for_x(&mut e, case, 0);
        assert_eq!(
            e.state.turn_history.current.creatures_died, 1,
            "{}: exactly one creature died (CR 704.5f)",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: a 0/0 with no counters dies as a state-based action (CR 704.5f)",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&source),
            "{}: it goes to its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn q86_x_cards_require_the_exact_printed_cost() {
    for (index, case) in cases().iter().enumerate() {
        // Shifting Wall prints {X} with no colored pip, so colorless mana is complete for it.
        if case.id == "shifting_wall" {
            let mut e = engine(20_884_000 + index as u64);
            let source = cast_for_x(&mut e, case, 2);
            assert_eq!(plus_counters(&e, source), 2);
            assert_eq!(body(&e, source), (Some(2), Some(2)));
            continue;
        }
        let mut e = engine(20_884_000 + index as u64);
        // Supply the whole generic total for X=2 (X plus every colored pip) but no colored mana,
        // so the cast can only fail on the missing printed pip and not on an insufficient amount.
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2 + case.pips,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell_x(slot, vec![], 2))
                .is_err(),
            "{}: the printed colored pip is required",
            case.id
        );
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 2 + case.pips),
            "{}: a rejected cast spends nothing",
            case.id
        );
    }
}
