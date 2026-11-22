//! Actual-card scenarios for the reviewed Puncturing Blow damage-and-exile family.
//!
//! Expectations are printed Oracle values. CR 115.1 provides one creature target;
//! CR 120.2/120.3 apply the damage; CR 614.6 and the saved rulings make the target's
//! first death this turn an exile instead, even when damage was prevented or the death
//! came from another cause.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_event::Ev;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: u32,
    sorcery: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "magma_spray",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            amount: 2,
            sorcery: false,
        },
        Case {
            id: "puncturing_blow",
            mana: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
            amount: 5,
            sorcery: true,
        },
        Case {
            id: "reduce_to_ashes",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            amount: 5,
            sorcery: true,
        },
        Case {
            id: "scorchmark",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            amount: 2,
            sorcery: false,
        },
    ]
}

fn fresh_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
    GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine")
}

fn engine(seed: u64) -> GameEngine {
    let mut e = fresh_engine(seed);
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    slot
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

#[test]
fn puncturing_blow_batch_cards_deal_their_printed_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(820_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
        let slot = cast_at(&mut e, &case, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}: six toughness survives",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].damage, case.amount,
            "{}: printed damage",
            case.id
        );
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn puncturing_blow_batch_cards_exile_a_lethally_damaged_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(821_000 + index as u64);
        let target =
            inject_creature_with_stats(&mut e, 1, "grizzly_bears", case.amount, case.amount);
        cast_at(&mut e, &case, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: lethal damage exiles",
            case.id
        );
        assert_eq!(
            e.state.turn_history.current.creatures_died, 0,
            "{}: no dies event",
            case.id
        );
    }
}

#[test]
fn puncturing_blow_batch_cards_exile_a_later_death_after_prevented_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(822_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 6, 6);
        e.state.add_damage_prevention_shield(target, case.amount);
        cast_at(&mut e, &case, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].damage, 0, "{}", case.id);

        inject_card_into_hand(&mut e, 0, "village_rites");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "village_rites");
        e.apply_scenario_command(
            0,
            &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, target)]),
        )
        .unwrap_or_else(|error| panic!("{}: sacrifice: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: prevented damage still marks the target for exile",
            case.id
        );
        assert_eq!(
            e.state.turn_history.current.creatures_died, 0,
            "{}: no dies event",
            case.id
        );
    }
}

#[test]
fn puncturing_blow_batch_cards_reject_a_land_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(823_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let pool_before = mana_pool(&e);
        let error = e
            .apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
            .expect_err("a land is not a creature target");
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: creature-only target: {error:?}",
            case.id
        );
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert_eq!(mana_pool(&e), pool_before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn puncturing_blow_batch_sorceries_reject_instant_timing() {
    for (index, case) in cases().into_iter().filter(|case| case.sorcery).enumerate() {
        let mut e = fresh_engine(824_000 + index as u64);
        advance_to_declare_attackers(&mut e);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let pool_before = mana_pool(&e);
        let error = e
            .apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
            .expect_err("a sorcery cannot be cast in the declare attackers step");
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: sorcery timing: {error:?}",
            case.id
        );
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert_eq!(mana_pool(&e), pool_before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

fn mana_pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn puncturing_blow_batch_cards_fizzle_when_the_target_leaves_before_resolution() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(825_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let slot = cast_at(&mut e, &case, target);
        e.state.players[1]
            .battlefield
            .retain(|candidate| *candidate != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;

        let batch = pass_both_players_with_batch(&mut e);
        assert!(
            batch.events.iter().any(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text.contains("fizzles")
            )),
            "{}: the engine reports the CR 608.2b fizzle",
            case.id
        );
        assert!(
            !batch.events.iter().any(|event| matches!(
                &event.ev,
                Some(Ev::PermanentMoved(moved))
                    if moved.object_id == target
                        && moved.destination
                            == tricerules_proto::ruled::v1::permanent_moved::Destination::Exile
                                as i32
            )),
            "{}: no exile replacement is applied",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: the stale target stays in the graveyard",
            case.id
        );
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the fizzled spell is in its owner's graveyard",
            case.id
        );
        assert_eq!(
            e.state.turn_history.current.creatures_died, 0,
            "{}: no damage or replacement is applied",
            case.id
        );
    }
}
