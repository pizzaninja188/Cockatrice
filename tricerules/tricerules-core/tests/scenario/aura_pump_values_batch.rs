//! Actual-card scenarios for the reviewed Aura power/toughness family.
//!
//! Expectations are printed Oracle values. CR 303.4a makes the Aura target the creature it enchants; CR 613.4c applies the attached P/T modifier; CR 704.5m sends an unattached Aura to the graveyard and its modifier ends.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump_power: i32,
    pump_toughness: i32,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "divine_transformation",
            mana: ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
            pump_power: 3,
            pump_toughness: 3,
        },
        Case {
            id: "feast_of_the_unicorn",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            pump_power: 4,
            pump_toughness: 0,
        },
        Case {
            id: "giant_strength",
            mana: ManaGift {
                r: 2,
                ..Default::default()
            },
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "hardened-scale_armor",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            pump_power: 3,
            pump_toughness: 3,
        },
        Case {
            id: "heros_resolve",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            pump_power: 1,
            pump_toughness: 5,
        },
        Case {
            id: "knights_pledge",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "siegecraft",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            pump_power: 2,
            pump_toughness: 4,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[card, "savannah_lions", "grizzly_bears", "disenchant"],
        ),
        deck_with("forest", &["grizzly_bears"]),
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

fn copy_mana(m: &ManaGift) -> ManaGift {
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
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn begin_cast_aura(e: &mut GameEngine, card_id: &str, mana: ManaGift, target: u32) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, card_id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    let before = pool(e);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    assert_ne!(pool(e), before, "{card_id}: the cast consumed mana");
    assert_eq!(
        e.state.stack.len(),
        1,
        "{card_id}: one Aura spell on the stack"
    );
    slot
}

fn cast_aura(e: &mut GameEngine, card_id: &str, mana: ManaGift, target: u32) -> u32 {
    begin_cast_aura(e, card_id, mana, target);
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, card_id)
}

#[test]
fn aura_pump_values_batch_cards_attach_and_apply_the_printed_power_and_toughness() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_600_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let aura = cast_aura(&mut e, case.id, case.mana, creature);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(creature)),
            "{}: the Aura is attached",
            case.id
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some(2 + case.pump_power as u32),
                Some(2 + case.pump_toughness as u32)
            ),
            "{}: exact printed pump",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: no life change", case.id);
    }
}

#[test]
fn aura_pump_values_batch_cards_affect_only_the_enchanted_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_610_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let other = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        cast_aura(&mut e, case.id, case.mana, creature);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some(2 + case.pump_power as u32),
                Some(2 + case.pump_toughness as u32)
            ),
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(other), e.effective_toughness(other)),
            (Some(2), Some(1)),
            "{}: the unrelated creature is unchanged",
            case.id
        );
    }
}

#[test]
fn aura_pump_values_batch_cards_remove_the_modifier_when_the_aura_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_620_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let aura = cast_aura(&mut e, case.id, case.mana, creature);
        assert_eq!(
            e.effective_power(creature),
            Some(2 + case.pump_power as u32),
            "{}",
            case.id
        );
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
            e.effective_power(creature),
            Some(2),
            "{}: power restored",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(2),
            "{}: toughness restored",
            case.id
        );
    }
}

#[test]
fn aura_pump_values_batch_cards_reject_a_noncreature_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_630_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, copy_mana(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = e.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == slot)
            .expect("the injected card is in hand");
        let before_pool = pool(&e);
        let before_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal Aura target",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand[hand_slot], slot,
            "{}: rejected Aura stays in hand",
            case.id
        );
        assert_eq!(pool(&e), before_pool, "{}: no mana is spent", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: nothing is on the stack",
            case.id
        );
        assert_eq!(
            e.state.command_index, before_index,
            "{}: no command position change",
            case.id
        );
    }
}

#[test]
fn aura_pump_values_batch_cards_fizzle_when_the_target_left() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_640_000 + index as u64, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        let slot = begin_cast_aura(&mut e, case.id, copy_mana(&case.mana), target);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the Aura spell fizzles",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&slot),
            "{}: fizzled Aura in owner's graveyard",
            case.id
        );
        assert!(
            !e.state.players[1].graveyard.contains(&slot),
            "{}: not in opponent's graveyard",
            case.id
        );
    }
}

#[test]
fn aura_pump_values_batch_cards_attached_aura_follows_its_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_650_000 + index as u64, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        let aura = cast_aura(&mut e, case.id, copy_mana(&case.mana), target);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Graveyard,
            "{}: the attached Aura follows",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&aura),
            "{}: Aura in owner's graveyard",
            case.id
        );
    }
}
