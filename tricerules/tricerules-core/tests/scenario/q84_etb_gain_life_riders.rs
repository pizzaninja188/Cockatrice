//! Actual-card scenarios for the queue-v84 03-etb-gain-life-riders cohort.
//!
//! Every card gains life when it enters and has one printed rider. Expectations are printed Oracle
//! values. CR 603.6a stages the entry trigger, CR 119.3 adjusts the controller's life total, and CR
//! 702.51a-c let convoke tap an untapped creature of the matching color for each colored mana and
//! any untapped creature you control for each generic mana.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, CastMethod, RuledCommand};

struct Case {
    id: &'static str,
    life: i32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "elite_guardmage",
            life: 3,
        },
        Case {
            id: "loxodon_restorer",
            life: 4,
        },
        Case {
            id: "circuit_mender",
            life: 2,
        },
    ]
}

fn engine(seed: u64, p0_specials: &[&str], p1_specials: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", p0_specials),
        deck_with("forest", p1_specials),
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

fn cast_card(e: &mut GameEngine, case: &Case, mana: ManaGift) -> u32 {
    give_mana(e, 0, gift(&mana));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let creature = battlefield_object_for_card(e, 0, case.id);
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
        "{}: exact cast",
        case.id
    );
    creature
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

/// `convoked` pairs each tapped creature with its contribution kind: 1 white, 2 blue, 3 black,
/// 4 red, 5 green, 6 generic (the wire `ObjectPaymentKind` spellings for CR 702.51a).
fn draft_convoke(
    engine: &GameEngine,
    card: &str,
    convoked: &[(u32, i32)],
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
                .map(|&(oid, kind)| rv1::ObjectPaymentContribution {
                    object: Some(object_ref(engine, oid)),
                    kind,
                })
                .collect(),
            mana: Some(mana),
        }),
        ..Default::default()
    }
}

#[test]
fn q84_gain_life_canary_simplest_elite_guardmage_gains_three_and_draws() {
    let case = &cases()[0];
    let mut e = engine(84_030_001, &[case.id], &[]);
    let life = e.state.players[0].life;
    let library = e.state.players[0].library.len();
    let hand = e.state.players[0].hand.len();
    let creature = cast_card(
        &mut e,
        case,
        ManaGift {
            w: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(e.state.players[0].life, life + 3, "printed entry life gain");
    assert_eq!(
        e.state.players[0].library.len(),
        library - 1,
        "the same trigger draws one card"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand + 1,
        "the trigger draws one card"
    );
    assert!(e.effective_has_keyword(creature, Keyword::Flying));
    assert_eq!(e.effective_power(creature), Some(2));
    assert_eq!(e.effective_toughness(creature), Some(3));
}

#[test]
fn q84_gain_life_canary_hardest_loxodon_restorer_convokes_and_gains_four() {
    let case = &cases()[1];
    let mut e = engine(84_031_001, &[case.id], &[]);
    ensure_in_hand(&mut e, 0, case.id);
    let mut convoked = vec![];
    // CR 702.51a: white creatures pay the two white pips, any creatures pay the four generic.
    for _ in 0..2 {
        let oid = inject_creature_on_battlefield(&mut e, 0, "alaborn_grenadier");
        convoked.push((oid, 1));
    }
    for _ in 0..4 {
        let oid = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        convoked.push((oid, 6));
    }
    let life = e.state.players[0].life;
    let cast = draft_convoke(&e, case.id, &convoked, rv1::PaymentMana::default());
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(cast)),
        },
    )
    .expect("convoke pays the whole printed cost");
    assert_eq!(e.state.stack.len(), 1, "one creature spell");
    resolve_entire_stack_two_player(&mut e);
    let creature = battlefield_object_for_card(&e, 0, case.id);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    for (oid, _) in &convoked {
        assert!(
            e.state.objects[oid].tapped,
            "every convoked creature tapped"
        );
    }
    assert_eq!(e.state.players[0].life, life + 4, "printed entry life gain");
    assert!(e.effective_has_keyword(creature, Keyword::Convoke));
    assert_eq!(e.effective_power(creature), Some(3));
    assert_eq!(e.effective_toughness(creature), Some(4));
}

#[test]
fn q84_loxodon_restorer_convoke_needs_white_creatures_for_white_pips() {
    let case = &cases()[1];
    let mut e = engine(84_032_002, &[case.id], &[]);
    ensure_in_hand(&mut e, 0, case.id);
    let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
    let mut convoked = vec![];
    for _ in 0..6 {
        let oid = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        convoked.push((oid, 5));
    }
    let before = format!("{:?}", e.state);
    let cast = draft_convoke(&e, case.id, &convoked, rv1::PaymentMana::default());
    assert!(
        e.apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(cast))
            }
        )
        .is_err(),
        "six green creatures cannot pay the two white pips"
    );
    assert_eq!(format!("{:?}", e.state), before, "rejected cast is atomic");
    assert_eq!(e.state.objects[&card].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q84_gain_life_cards_apply_printed_amount_and_controller_scope() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(84_033_000 + index as u64, &[case.id], &[]);
        let own_life = e.state.players[0].life;
        let other_life = e.state.players[1].life;
        move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life,
            own_life + case.life,
            "{}: printed life gain",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, other_life,
            "{}: the opponent does not gain",
            case.id
        );
    }
}

#[test]
fn q84_gain_life_cards_do_not_trigger_from_other_entries() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(84_034_000 + index as u64, &[case.id, "grizzly_bears"], &[]);
        move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        let life_after_self = e.state.players[0].life;
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(e.state.stack.len(), 0, "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, life_after_self,
            "{}: another creature's entry does not gain life",
            case.id
        );
    }
}

#[test]
fn q84_circuit_mender_draws_when_exiled_because_it_left_the_battlefield() {
    let case = &cases()[2];
    let mut e = engine(84_035_001, &[case.id, "swords_to_plowshares"], &[]);
    let creature = cast_card(
        &mut e,
        case,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut e, 0, "swords_to_plowshares");
    let hand_before = e.state.players[0].hand.len();
    let hand_before_swords = hand_before;
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "swords_to_plowshares");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("exile Circuit Mender");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Exile,
        "Swords exiles rather than destroys"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_swords,
        "cast Swords, then the leaves trigger draws one"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "the leaves trigger draws one card"
    );
}
