//! Actual-card scenarios for the queue-v23 haste self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 602.2 governs the activated pump,
//! CR 611.2c/613.4c apply the source-only modifier, CR 514.2 ends it, and
//! CR 702.10 grants haste.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    mana: ManaGift,
    activation: ManaGift,
    body: (u32, u32),
    pump: (i32, i32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "crypt_ripper",
            mana: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
            activation: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (2, 2),
            pump: (1, 1),
        },
        Case {
            id: "furnace_spirit",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (1, 1),
            pump: (1, 0),
        },
        Case {
            id: "pyre_charger",
            mana: ManaGift {
                r: 2,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (1, 1),
            pump: (1, 0),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let _slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn haste_self_pump_batch_canary_cards_cast_and_apply_the_printed_keyword() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "crypt_ripper" | "pyre_charger"))
    {
        let mut e = engine(12_600_000);
        let source = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(source, Keyword::Haste),
            "{}: printed keyword",
            case.id
        );
        assert!(
            e.state.objects[&source].summoning_sick,
            "{}: freshly cast",
            case.id
        );
    }
}

#[test]
fn haste_self_pump_batch_cards_cast_with_exact_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_610_000 + index as u64);
        let source = cast_source(&mut e, &case);
        assert_eq!(e.state.objects[&source].controller, 0, "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}

#[test]
fn haste_self_pump_batch_cards_activate_only_the_source_with_the_printed_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_620_000 + index as u64);
        let source = cast_source(&mut e, &case);
        let bystander = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some((case.body.0 as i32 + case.pump.0) as u32),
                Some((case.body.1 as i32 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(3), Some(3)),
            "{}: source only",
            case.id
        );
    }
}

#[test]
fn haste_self_pump_batch_cards_stack_repeated_activations() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_630_000 + index as u64);
        let source = cast_source(&mut e, &case);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: first activation: {error:?}", case.id));
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: first activation on the stack",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: first payment", case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: no pump before resolution",
            case.id
        );
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: second activation: {error:?}", case.id));
        assert_eq!(
            e.state.stack.len(),
            2,
            "{}: both activations are pending",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: second payment", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some((case.body.0 as i32 + 2 * case.pump.0) as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some((case.body.1 as i32 + 2 * case.pump.1) as u32),
            "{}",
            case.id
        );
    }
}

#[test]
fn haste_self_pump_batch_cards_reject_activation_without_the_exact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_640_000 + index as u64);
        let source = cast_source(&mut e, &case);
        let before = (e.effective_power(source), e.effective_toughness(source));
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: no mana means no activation",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            before,
            "{}: rejected activation leaves the body unchanged",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn haste_self_pump_batch_cards_expire_the_pump_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_650_000 + index as u64);
        let source = cast_source(&mut e, &case);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate");
        resolve_entire_stack_two_player(&mut e);
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn haste_self_pump_batch_cards_attack_immediately_after_being_cast() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_660_000 + index as u64);
        let source = cast_source(&mut e, &case);
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("haste lets the freshly cast creature attack");
    }
}
