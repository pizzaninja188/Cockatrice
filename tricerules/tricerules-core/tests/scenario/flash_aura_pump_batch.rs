//! Actual-card scenarios for the queue-v18 flash Aura pump cohort.
//!
//! Expectations are printed Oracle values. CR 702.8a permits flash timing; CR 303.4a
//! defines the Aura target and attachment; CR 613.4c applies the attached P/T
//! modifier; CR 704.5m ends it when the Aura leaves.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "feral_invocation",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            pump: (2, 2),
        },
        Case {
            id: "gift_of_granite",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            pump: (0, 2),
        },
        Case {
            id: "indomitable_will",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            pump: (1, 2),
        },
        Case {
            id: "magetas_boon",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            pump: (1, 2),
        },
        Case {
            id: "stoneskin",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            pump: (0, 10),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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
    let _slot = inject_card_into_hand(e, 0, card_id);
    let hand_slot = hand_index_for_card(e, 0, card_id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, card_id)
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn flash_aura_pump_batch_cards_attach_and_apply_the_printed_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_820_000 + index as u64);
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
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: exact printed pump",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(3), Some(3)),
            "{}: only the enchanted creature",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    }
}

#[test]
fn flash_aura_pump_batch_cards_use_flash_timing_on_an_opponent_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_821_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        end_active_turn(&mut e, 0);
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.priority_player_id(),
            e.state.players[1].id,
            "{}: opponent has priority",
            case.id
        );
        e.apply_scenario_command(e.state.players[1].id, &pass())
            .expect("opponent passes priority");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{}: flash cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Battlefield,
            "{}: flash resolved on the opponent's turn",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some((2 + case.pump.1) as u32),
            "{}: the pump applied",
            case.id
        );
    }
}

#[test]
fn flash_aura_pump_batch_cards_reject_a_noncreature_and_fizzle_a_stale_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_822_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = pool(&e);
        let _slot = inject_card_into_hand(&mut e, 0, case.id);
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

        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
        give_mana(&mut e, 0, gift(&case.mana));
        let _slot = inject_card_into_hand(&mut e, 0, case.id);
        let fizzle_hand = hand_index_for_card(&e, 0, case.id);
        let fizzle_slot = e.state.players[0].hand[fizzle_hand];
        e.apply_scenario_command(0, &cast_spell(fizzle_hand, target_object(target)))
            .expect("cast at the creature");
        move_to_graveyard(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&fizzle_slot].zone,
            Zone::Graveyard,
            "{}: the Aura spell fizzles",
            case.id
        );
        assert_eq!(
            e.state.objects[&fizzle_slot].attached_to, None,
            "{}: never attached",
            case.id
        );
    }
}

#[test]
fn flash_aura_pump_batch_cards_remove_the_pump_when_the_aura_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_823_000 + index as u64);
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
        let _slot = inject_card_into_hand(&mut e, 0, "disenchant");
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
    }
}

#[test]
fn flash_aura_pump_batch_cards_keep_the_static_modifier_after_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_824_000 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let aura = cast_aura(&mut e, case.id, gift(&case.mana), creature);
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: the static Aura pump remains",
            case.id
        );
    }
}
