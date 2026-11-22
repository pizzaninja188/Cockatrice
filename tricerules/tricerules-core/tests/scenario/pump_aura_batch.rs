//! Actual-card scenarios for the reviewed negative-pump Aura family.
//!
//! Expectations are printed Oracle values. CR 303.4 governs the Aura attach target and the Aura's
//! state-based cleanup when it is unattached; CR 613.4c applies the layer-7c P/T modifier; CR
//! 704.5m sends an Aura to the graveyard when the permanent it enchants leaves the battlefield.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    delta_power: i32,
    delta_toughness: i32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "sensory_deprivation",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            delta_power: -3,
            delta_toughness: 0,
        },
        Case {
            id: "chant_of_the_skifsang",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            delta_power: -13,
            delta_toughness: 0,
        },
        Case {
            id: "clinging_darkness",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            delta_power: -4,
            delta_toughness: -1,
        },
        Case {
            id: "debilitating_injury",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            delta_power: -2,
            delta_toughness: -2,
        },
        Case {
            id: "enfeeblement",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
            delta_power: -2,
            delta_toughness: -2,
        },
        Case {
            id: "mires_grasp",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            delta_power: -3,
            delta_toughness: -3,
        },
        Case {
            id: "pin_to_the_earth",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            delta_power: -6,
            delta_toughness: 0,
        },
        Case {
            id: "torment",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            delta_power: -3,
            delta_toughness: 0,
        },
        Case {
            id: "weakness",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            delta_power: -2,
            delta_toughness: -1,
        },
        Case {
            id: "weight_of_the_underworld",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            delta_power: -3,
            delta_toughness: -2,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn assert_pool_empty(e: &GameEngine, id: &str) {
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost consumed");
}

fn expected(value: u32, delta: i32) -> u32 {
    (i32::try_from(value).expect("small printed value") + delta).max(0) as u32
}

fn begin_cast_aura(e: &mut GameEngine, id: &str, mana: ManaGift, target: u32) -> (u32, u32) {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    semantic::accepted(e, 0, &cast_spell(hand_slot, target_object(target)));
    assert_pool_empty(e, id);
    assert_eq!(e.state.stack.len(), 1, "{id}: one Aura spell on the stack");
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Stack,
        "{id}: the Aura card is on the stack"
    );
    (slot, e.state.stack[0].id)
}

fn cast_aura(e: &mut GameEngine, id: &str, mana: ManaGift, target: u32) -> (u32, u32) {
    let (slot, _) = begin_cast_aura(e, id, mana, target);
    resolve_entire_stack_two_player(e);
    let aura = battlefield_object_for_card(e, 0, id);
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(target)),
        "{id}: the Aura enters attached to its target"
    );
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{id}: the Aura resolves onto the battlefield"
    );
    (slot, aura)
}

#[test]
fn pump_aura_batch_cards_cast_and_modify_the_enchanted_creature() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(750_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        let (_, aura) = cast_aura(&mut e, case.id, case.mana, target);
        assert_eq!(
            e.effective_power(target),
            Some(expected(20, case.delta_power)),
            "{}: exact power modifier",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(expected(20, case.delta_toughness)),
            "{}: exact toughness modifier",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].controller, e.state.players[1].id,
            "{}: the enchanted creature remains under its controller",
            case.id
        );
        assert_eq!(
            e.state.objects[&aura].owner, e.state.players[0].id,
            "{}: the Aura is owned by its caster",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: opponent hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: opponent library is unchanged",
            case.id
        );
    }
}

#[test]
fn pump_aura_batch_cards_persist_after_cleanup() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(751_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        let (_, aura) = cast_aura(&mut e, case.id, case.mana, target);
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Battlefield,
            "{}: the Aura remains on the battlefield",
            case.id
        );
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(target)),
            "{}: the Aura remains attached",
            case.id
        );
        assert_eq!(
            e.effective_power(target),
            Some(expected(20, case.delta_power)),
            "{}: the modifier persists past cleanup",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(expected(20, case.delta_toughness)),
            "{}: the toughness modifier persists past cleanup",
            case.id
        );
    }
}

#[test]
fn pump_aura_batch_cards_reject_a_noncreature_target() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(752_000 + case_index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let before_pool = pool(&e);
        let before_command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal Aura target",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand[hand_slot], slot,
            "{}: the rejected Aura stays in hand",
            case.id
        );
        assert_eq!(pool(&e), before_pool, "{}: no mana is spent", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: nothing is on the stack",
            case.id
        );
        assert_eq!(
            e.state.command_index, before_command_index,
            "{}: the rejected cast changes no command position",
            case.id
        );
    }
}

#[test]
fn pump_aura_batch_cards_fizzle_when_the_target_left() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(753_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        let (slot, _) = begin_cast_aura(&mut e, case.id, case.mana, target);
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
            "{}: the fizzled Aura is in its owner's graveyard",
            case.id
        );
        assert!(
            !e.state.players[1].graveyard.contains(&slot),
            "{}: the fizzled Aura is not in the opponent's graveyard",
            case.id
        );
    }
}

#[test]
fn pump_aura_batch_attached_aura_follows_its_creature_to_the_graveyard() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(754_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        let (_, aura) = cast_aura(&mut e, case.id, case.mana, target);
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
            "{}: an unattached Aura is put into its owner's graveyard",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&aura),
            "{}: the Aura is in its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn chant_of_the_skifsang_floors_power_at_zero() {
    let mut e = engine(755_000);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_aura(
        &mut e,
        "chant_of_the_skifsang",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        target,
    );
    assert_eq!(e.effective_power(target), Some(0));
    assert_eq!(e.effective_toughness(target), Some(2));
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
