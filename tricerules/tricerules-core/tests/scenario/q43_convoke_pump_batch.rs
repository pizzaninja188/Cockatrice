//! Actual-card scenarios for the queue-v43 Convoke pump cohort.
//!
//! Expectations are printed Oracle values. CR 702.51a-c let tapped creatures pay generic or a
//! matching color; CR 601.2h fixes payment order; CR 115.1 chooses one target creature and
//! CR 611.2a/613.4c apply the printed pump until end of turn.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, CastMethod};

struct Case {
    id: &'static str,
    generic: u32,
    mana: ManaGift,
    dp: i32,
    dt: i32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "packs_favor",
            generic: 2,
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            dp: 3,
            dt: 3,
        },
        Case {
            id: "gather_courage",
            generic: 0,
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            dp: 2,
            dt: 2,
        },
        Case {
            id: "collective_nightmare",
            generic: 2,
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            dp: -3,
            dt: -3,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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
    convoked: &[(u32, rv1::ObjectPaymentKind)],
    mana: &ManaGift,
    target: u32,
) -> rv1::CastSpell {
    let hand = hand_index_for_card(engine, 0, card);
    rv1::CastSpell {
        cast_method: CastMethod::Normal as i32,
        source: Some(hand_cast_source(hand)),
        targets: target_object(target),
        payment: Some(rv1::PaymentSelection {
            waterbend: vec![],
            source: Some(object_ref(engine, engine.state.players[0].hand[hand])),
            expected_state_revision: engine.state.command_index,
            convoke: convoked
                .iter()
                .map(|&(oid, kind)| rv1::ObjectPaymentContribution {
                    object: Some(object_ref(engine, oid)),
                    kind: kind as i32,
                })
                .collect(),
            mana: Some(rv1::PaymentMana {
                w: mana.w,
                u: mana.u,
                b: mana.b,
                r: mana.r,
                g: mana.g,
                c: mana.c,
            }),
        }),
        ..Default::default()
    }
}

fn cast_pump(e: &mut GameEngine, case: &Case, target: u32) {
    ensure_in_hand(e, 0, case.id);
    let mut convoked = vec![];
    for _ in 0..case.generic {
        convoked.push((
            inject_creature_on_battlefield(e, 0, "grizzly_bears"),
            rv1::ObjectPaymentKind::Generic,
        ));
    }
    give_mana(e, 0, gift(&case.mana));
    let cast = draft(e, case.id, &convoked, &case.mana, target);
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(cast)),
        },
    )
    .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    for (oid, _) in &convoked {
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
        "{}: exact colored payment",
        case.id
    );
    assert!(
        e.state.objects[&target].zone == Zone::Battlefield,
        "{}: the target stays",
        case.id
    );
}

#[test]
fn q43_convoke_pump_canary_uses_convoke_and_pumps() {
    let case = &cases()[0];
    let mut e = engine(43_040_001, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_pump(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(5), Some(5)),
        "Pack's Favor applies its printed +3/+3"
    );
}

#[test]
fn q43_convoke_pump_cards_apply_their_printed_values() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_041_000 + index as u64, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
        cast_pump(&mut e, case, target);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some((4 + case.dp) as u32), Some((4 + case.dt) as u32)),
            "{}: printed pump",
            case.id
        );
    }
}

#[test]
fn q43_convoke_pump_cards_reject_a_generic_contribution_and_illegal_targets() {
    let mut e = engine(43_042_001, "gather_courage");
    ensure_in_hand(&mut e, 0, "gather_courage");
    let tapped = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let cast = draft(
        &e,
        "gather_courage",
        &[(tapped, rv1::ObjectPaymentKind::Generic)],
        &ManaGift {
            g: 1,
            ..Default::default()
        },
        tapped,
    );
    assert!(
        e.apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(cast))
            }
        )
        .is_err(),
        "a generic convoke contribution cannot pay Gather Courage's colored pip"
    );
    assert!(
        !e.state.objects[&tapped].tapped,
        "no convoke payment consumed"
    );

    let mut player_e = engine(43_043_001, "packs_favor");
    inject_card_into_hand(&mut player_e, 0, "packs_favor");
    give_mana(
        &mut player_e,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&player_e, 0, "packs_favor");
    assert!(
        player_e
            .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .is_err(),
        "a player is not a creature target"
    );
}

#[test]
fn q43_convoke_pump_cards_accept_a_matching_color_contribution() {
    let mut e = engine(43_044_001, "gather_courage");
    ensure_in_hand(&mut e, 0, "gather_courage");
    let helper = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
    let cast = draft(
        &e,
        "gather_courage",
        &[(helper, rv1::ObjectPaymentKind::Green)],
        &ManaGift::default(),
        target,
    );
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(cast)),
        },
    )
    .expect("a green creature pays the green pip");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.objects[&helper].tapped,
        "convoke tapped the green creature"
    );
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(6), Some(6)),
        "Gather Courage applies its printed +2/+2"
    );
}
