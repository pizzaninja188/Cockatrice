//! Actual-card scenarios for the reviewed Convoke creature family.
//!
//! Expectations are printed Oracle values. CR 702.51a-c let each tapped creature pay
//! one generic mana or one mana of its color; CR 601.2h fixes the payment order.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, CastMethod, RuledCommand};

struct Case {
    id: &'static str,
    generic: u32,
    mana: ManaGift,
    body: (u32, u32),
    extra: Option<Keyword>,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "halo_hopper",
            generic: 3,
            mana: ManaGift::default(),
            body: (3, 2),
            extra: None,
        },
        Case {
            id: "ledev_guardian",
            generic: 3,
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            body: (2, 4),
            extra: None,
        },
        Case {
            id: "will-forged_golem",
            generic: 6,
            mana: ManaGift::default(),
            body: (4, 4),
            extra: None,
        },
        Case {
            id: "conclave_equenaut",
            generic: 4,
            mana: ManaGift {
                w: 2,
                ..Default::default()
            },
            body: (3, 3),
            extra: Some(Keyword::Flying),
        },
        Case {
            id: "flight_of_equenauts",
            generic: 7,
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            body: (4, 5),
            extra: Some(Keyword::Flying),
        },
        Case {
            id: "thunderhead_squadron",
            generic: 5,
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (3, 4),
            extra: Some(Keyword::Flying),
        },
        Case {
            id: "autochthon_wurm",
            generic: 10,
            mana: ManaGift {
                w: 2,
                g: 3,
                ..Default::default()
            },
            body: (9, 14),
            extra: Some(Keyword::Trample),
        },
        Case {
            id: "siege_wurm",
            generic: 5,
            mana: ManaGift {
                g: 2,
                ..Default::default()
            },
            body: (5, 5),
            extra: Some(Keyword::Trample),
        },
        Case {
            id: "guardian_of_vitu-ghazi",
            generic: 6,
            mana: ManaGift {
                w: 1,
                g: 1,
                ..Default::default()
            },
            body: (4, 7),
            extra: Some(Keyword::Vigilance),
        },
        Case {
            id: "rosemane_centaur",
            generic: 3,
            mana: ManaGift {
                w: 1,
                g: 1,
                ..Default::default()
            },
            body: (4, 4),
            extra: Some(Keyword::Vigilance),
        },
        Case {
            id: "arboretum_elemental",
            generic: 7,
            mana: ManaGift {
                g: 2,
                ..Default::default()
            },
            body: (7, 5),
            extra: Some(Keyword::Hexproof),
        },
        Case {
            id: "impervious_greatwurm",
            generic: 7,
            mana: ManaGift {
                g: 3,
                ..Default::default()
            },
            body: (16, 16),
            extra: Some(Keyword::Indestructible),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "grizzly_bears"]),
        deck_with("forest", &[]),
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

fn object_ref(engine: &GameEngine, oid: u32) -> rv1::CostObjectRef {
    rv1::CostObjectRef {
        object_id: oid,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0),
    }
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

fn draft(
    engine: &GameEngine,
    card: &str,
    convoked: &[u32],
    mana: rv1::PaymentMana,
) -> rv1::CastSpell {
    let hand = hand_index_for_card(engine, 0, card);
    rv1::CastSpell {
        cast_method: CastMethod::Normal as i32,
        source: Some(hand_cast_source(hand)),
        payment: Some(rv1::PaymentSelection {
            waterbend: vec![],
            source: Some(object_ref(engine, engine.state.players[0].hand[hand])),
            expected_state_revision: engine.state.command_index,
            convoke: convoked
                .iter()
                .map(|&oid| rv1::ObjectPaymentContribution {
                    object: Some(object_ref(engine, oid)),
                    kind: rv1::ObjectPaymentKind::Generic as i32,
                })
                .collect(),
            mana: Some(mana),
        }),
        ..Default::default()
    }
}

fn cast_with_convoke(e: &mut GameEngine, case: &Case) -> (u32, Vec<u32>) {
    ensure_in_hand(e, 0, case.id);
    let mut convoked = vec![];
    for _ in 0..case.generic {
        convoked.push(inject_creature_on_battlefield(e, 0, "grizzly_bears"));
    }
    give_mana(e, 0, gift(&case.mana));
    let cast = draft(
        e,
        case.id,
        &convoked,
        rv1::PaymentMana {
            w: case.mana.w,
            u: case.mana.u,
            b: case.mana.b,
            r: case.mana.r,
            g: case.mana.g,
            c: case.mana.c,
        },
    );
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(cast)),
        },
    )
    .expect("cast with exact convoke and colored mana");
    assert_eq!(e.state.stack.len(), 1, "{}: one creature spell", case.id);
    resolve_entire_stack_two_player(e);
    let creature = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    for oid in &convoked {
        assert!(
            e.state.objects[oid].tapped,
            "{}: convoke tapped its creature",
            case.id
        );
    }
    let pool = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "{}: colored mana was spent exactly",
        case.id
    );
    (creature, convoked)
}

#[test]
fn convoke_batch_cards_cast_with_convoke_and_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_530_000 + index as u64, case.id);
        let (creature, _) = cast_with_convoke(&mut e, &case);
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.body.1),
            "{}",
            case.id
        );
        if let Some(keyword) = case.extra {
            assert!(
                e.effective_has_keyword(creature, keyword),
                "{}: printed keyword remains active",
                case.id
            );
        }
    }
}

#[test]
fn convoke_batch_cards_require_colored_mana_and_keep_printed_keywords() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_531_000 + index as u64, case.id);
        let (creature, _) = cast_with_convoke(&mut e, &case);
        if let Some(keyword) = case.extra {
            assert!(
                e.effective_has_keyword(creature, keyword),
                "{}: printed keyword after resolution",
                case.id
            );
        }
    }

    // Convoke pays generic, never a colored pip. Siege Wurm needs {G}{G} even
    // with five tapped creatures available.
    let mut e = engine(9_532_000, "siege_wurm");
    ensure_in_hand(&mut e, 0, "siege_wurm");
    let card = e.state.players[0].hand[hand_index_for_card(&e, 0, "siege_wurm")];
    let mut convoked = vec![];
    for _ in 0..5 {
        convoked.push(inject_creature_on_battlefield(&mut e, 0, "grizzly_bears"));
    }
    let before = format!("{:?}", e.state);
    let cast = draft(&e, "siege_wurm", &convoked, rv1::PaymentMana::default());
    assert!(
        e.apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(cast))
            }
        )
        .is_err(),
        "generic convoke cannot pay the green pips"
    );
    assert_eq!(format!("{:?}", e.state), before, "rejected cast is atomic");
    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Hand,
        "the uncast card stays in hand"
    );
}
