//! Actual-card scenarios for the reviewed Mindstatic soft-counter family.
//!
//! Expectations are printed Oracle values. CR 701.6 counters a spell; CR 118.12/118.12a
//! treat "unless its controller pays" as an optional payment made as the spell resolves;
//! CR 608.2b fizzles a counter whose targeted spell left the stack. Force Spike and Mana
//! Tithe's saved 2004-10-04 rulings confirm the payment is optional.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::TargetRef;

struct Case {
    id: &'static str,
    mana: ManaGift,
    payment: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "force_spike",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            payment: 1,
        },
        Case {
            id: "mana_leak",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            payment: 3,
        },
        Case {
            id: "mana_tithe",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            payment: 1,
        },
        Case {
            id: "mindstatic",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            payment: 6,
        },
        Case {
            id: "quench",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            payment: 2,
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

fn spell_target(object_id: u32) -> TargetRef {
    TargetRef {
        counter_amount: 0,
        expected_zone_change_generation: None,
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        deck_with("island", &[card]),
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

fn cast_counter_over_bolt(e: &mut GameEngine, case: &Case) -> (u32, u32) {
    inject_card_into_hand(e, 0, "lightning_bolt");
    give_mana(
        e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_slot = hand_index_for_card(e, 0, "lightning_bolt");
    e.apply_scenario_command(0, &cast_spell(bolt_slot, target_player(1)))
        .expect("cast Lightning Bolt");
    let bolt = e.state.stack.last().expect("Bolt on stack").id;
    e.apply_scenario_command(0, &pass())
        .expect("active player passes");

    inject_card_into_hand(e, 1, case.id);
    give_mana(e, 1, gift(&case.mana));
    let counter_slot = hand_index_for_card(e, 1, case.id);
    e.apply_scenario_command(1, &cast_spell(counter_slot, vec![spell_target(bolt)]))
        .expect("cast the soft counter");
    let counter = e.state.stack.last().expect("counter on stack").id;
    (bolt, counter)
}

#[test]
fn mindstatic_batch_cards_decline_and_counter_the_exact_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(870_000 + index as u64, case.id);
        let (bolt, counter) = cast_counter_over_bolt(&mut e, &case);
        e.apply_scenario_command(1, &pass())
            .expect("counter controller passes");
        let parked = e
            .apply_scenario_command(0, &pass())
            .expect("target controller passes to resolve the counter");
        let choice = find_resolution_choice(&parked).expect("payment choice");
        assert_eq!(choice.generic_mana_cost, case.payment, "{}", case.id);
        assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
        assert!(
            !choice.payment_currently_legal,
            "{}: no mana was offered, so paying is not currently legal",
            case.id
        );
        e.apply_scenario_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .expect("decline the optional payment");
        assert!(e.state.players[0].graveyard.contains(&bolt), "{}", case.id);
        assert!(
            e.state.players[1].graveyard.contains(&counter),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn mindstatic_batch_cards_allow_the_targets_controller_to_pay() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(871_000 + index as u64, case.id);
        let (bolt, counter) = cast_counter_over_bolt(&mut e, &case);
        e.apply_scenario_command(1, &pass())
            .expect("counter controller passes");
        let parked = e
            .apply_scenario_command(0, &pass())
            .expect("target controller passes to resolve the counter");
        let choice = find_resolution_choice(&parked).expect("payment choice");
        assert_eq!(choice.generic_mana_cost, case.payment, "{}", case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: case.payment,
                ..Default::default()
            },
        );
        submit_mana_resolution_decision(
            &mut e,
            0,
            tricerules_proto::ruled::v1::ResolutionChoiceDecision::PayMana,
        )
        .expect("pay the exact generic amount");
        assert!(
            e.state.stack.iter().any(|item| item.id == bolt),
            "{}",
            case.id
        );
        assert!(
            !e.state.stack.iter().any(|item| item.id == counter),
            "{}",
            case.id
        );
        pass_both_players(&mut e);
        assert!(
            e.state.players[1].graveyard.contains(&counter),
            "{}",
            case.id
        );
        assert!(e.state.players[0].graveyard.contains(&bolt), "{}", case.id);
        assert_eq!(e.state.players[1].life, 17, "{}", case.id);
    }
}

#[test]
fn mindstatic_batch_cards_reject_a_cast_with_no_spell_on_the_stack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(872_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 1, case.id);
        give_mana(&mut e, 1, gift(&case.mana));
        let slot = hand_index_for_card(&e, 1, case.id);
        let pool_before = {
            let p = &e.state.players[1].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        };
        assert!(
            e.apply_scenario_command(1, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: a counter needs a spell target",
            case.id
        );
        let p = &e.state.players[1].mana_pool;
        assert_eq!(
            (p.white, p.blue, p.black, p.red, p.green, p.colorless),
            pool_before,
            "{}: the rejected cast spends no mana",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn mindstatic_batch_cards_fizzle_when_the_target_spell_already_left() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(873_000 + index as u64, case.id);
        let (bolt, counter) = cast_counter_over_bolt(&mut e, &case);
        e.state.stack.retain(|item| item.id != bolt);
        e.state.players[0].graveyard.push(bolt);
        e.state.objects.get_mut(&bolt).expect("bolt").zone = tricerules_core::Zone::Graveyard;
        *e.state.zone_change_generation.entry(bolt).or_insert(0) += 1;
        e.apply_scenario_command(1, &pass())
            .expect("counter controller passes");
        e.apply_scenario_command(0, &pass())
            .expect("target controller passes");
        assert!(
            e.state.players[1].graveyard.contains(&counter),
            "{}",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}
