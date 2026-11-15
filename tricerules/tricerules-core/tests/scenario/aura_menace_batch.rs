//! Actual-card scenarios for the reviewed menace Aura cohort.
//!
//! Expectations are printed Oracle values. CR 303.4a makes the Aura target the
//! creature it enchants; CR 613.4c applies the attached P/T modifier; CR 702.111a-b
//! makes the enchanted creature require two or more blockers; CR 704.5m ends the
//! modifier when the Aura leaves.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump_power: i32,
    pump_toughness: i32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "frenzied_rage",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            pump_power: 2,
            pump_toughness: 1,
        },
        Case {
            id: "furious_strength",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "giant_spectacle",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            pump_power: 2,
            pump_toughness: 1,
        },
        Case {
            id: "madcap_skills",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            pump_power: 3,
            pump_toughness: 0,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
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

fn cast_aura(e: &mut GameEngine, card_id: &str, mana: ManaGift, target: u32) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, card_id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, card_id)
}

#[test]
fn aura_menace_batch_cards_attach_and_apply_the_printed_pump_and_menace() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_650_000 + index as u64, case.id);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        let aura = cast_aura(&mut e, case.id, gift(&case.mana), creature);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(creature)),
            "{}: attached",
            case.id
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((2 + case.pump_power) as u32),
                Some((2 + case.pump_toughness) as u32)
            ),
            "{}: exact printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Menace),
            "{}: gained menace",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::Menace),
            "{}: only the enchanted creature",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(3), Some(3)),
            "{}: bystander unchanged",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}

#[test]
fn aura_menace_batch_cards_may_enchant_an_opponents_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_651_000 + index as u64, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        give_mana(&mut e, 0, gift(&case.mana));
        let _slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let legal = &e.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
            [&((hand_slot as u32) << 8)]
            .groups[0]
            .valid_permanent_ids;
        assert!(
            legal.contains(&own) && legal.contains(&opposing),
            "{}: every creature is a legal target",
            case.id
        );
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(opposing)))
            .expect("enchant the opponent's creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (
                Some((2 + case.pump_power) as u32),
                Some((2 + case.pump_toughness) as u32)
            ),
            "{}: the pump applies to the enchanted creature",
            case.id
        );
        assert!(
            e.effective_has_keyword(opposing, Keyword::Menace),
            "{}: the opponent's creature gains menace",
            case.id
        );
    }
}

#[test]
fn aura_menace_batch_cards_require_two_blockers_because_of_menace() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_652_000 + index as u64, case.id);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_aura(&mut e, case.id, gift(&case.mana), creature);
        let first = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![creature]))
            .expect("declare the enchanted attacker");
        e.apply_scenario_command(0, &pass()).expect("active pass");
        e.apply_scenario_command(1, &pass()).expect("defender pass");
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: creature,
                    blocker_id: first,
                }]),
            )
            .is_err(),
            "{}: one blocker disobeys menace",
            case.id
        );
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: creature,
                    blocker_id: first,
                },
                BlockPair {
                    attacker_id: creature,
                    blocker_id: second,
                },
            ]),
        )
        .expect("two blockers satisfy menace");
    }
}

#[test]
fn aura_menace_batch_cards_remove_the_pump_and_menace_when_the_aura_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_653_000 + index as u64, case.id);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let aura = cast_aura(&mut e, case.id, gift(&case.mana), creature);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "disenchant");
        let disenchant = hand_index_for_card(&e, 0, "disenchant");
        e.apply_scenario_command(0, &cast_spell(disenchant, target_object(aura)))
            .expect("destroy the Aura");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(2), Some(2)),
            "{}: pump removed",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, Keyword::Menace),
            "{}: menace removed",
            case.id
        );
    }
}

#[test]
fn aura_menace_batch_cards_reject_a_noncreature_target_and_fizzle_a_stale_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_654_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = pool(&e);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let rejected_slot = e.state.players[0].hand[hand_slot];
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal Aura target",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand[hand_slot], rejected_slot,
            "{}",
            case.id
        );
        assert_eq!(pool(&e), pool_before, "{}: no mana spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let fizzle_hand = hand_index_for_card(&e, 0, case.id);
        let fizzle_slot = e.state.players[0].hand[fizzle_hand];
        e.apply_scenario_command(0, &cast_spell(fizzle_hand, target_object(target)))
            .expect("cast at the creature");
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&fizzle_slot].zone,
            Zone::Graveyard,
            "{}: the Aura spell fizzles",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&fizzle_slot),
            "{}: fizzled Aura in owner's graveyard",
            case.id
        );
    }
}
