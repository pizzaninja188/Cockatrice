//! Actual-card scenarios for the queue-v32 +N/-N Aura cohort.
//!
//! Expectations are printed Oracle values. CR 303.4a attaches an Aura to the
//! creature it targets; CR 613.4c applies the modifier in layer 7c; CR 704.5m
//! sends an unattached Aura to the graveyard; CR 107.4e permits either hybrid half.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{AttachmentRecipient, Zone};

struct Case {
    id: &'static str,
    mana: ManaGift,
    delta: (i32, i32),
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "boon_of_emrakul",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            delta: (3, -3),
        },
        Case {
            id: "immolation",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            delta: (2, -2),
        },
        Case {
            id: "sangrite_backlash",
            mana: ManaGift {
                b: 1,
                r: 1,
                ..Default::default()
            },
            delta: (3, -3),
        },
        Case {
            id: "twisted_experiment",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            delta: (3, -1),
        },
    ]
}

fn gift(case: &Case) -> ManaGift {
    ManaGift {
        w: case.mana.w,
        u: case.mana.u,
        b: case.mana.b,
        r: case.mana.r,
        g: case.mana.g,
        c: case.mana.c,
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["revoke_existence"]),
        deck_with("island", &["swords_to_plowshares"]),
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

fn cast_aura(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    give_mana(e, 0, gift(case));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let aura = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        e.state.objects[&aura].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(target)),
        "{}",
        case.id
    );
    aura
}

#[test]
fn q32_plus_minus_auras_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(32_050_001);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
    cast_aura(&mut e, case, bear);
    assert_eq!(e.effective_power(bear), Some(7));
    assert_eq!(e.effective_toughness(bear), Some(3));
    assert_eq!(e.state.players[0].life, 20);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_plus_minus_auras_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(32_050_002);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
    cast_aura(&mut e, case, bear);
    assert_eq!(e.effective_power(bear), Some(8));
    assert_eq!(e.effective_toughness(bear), Some(2));
}

#[test]
fn q32_plus_minus_auras_cards_apply_their_printed_modifier() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_051_000 + index as u64);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        cast_aura(&mut e, case, bear);
        let expected_toughness = (5 + case.delta.1).max(0) as u32;
        assert_eq!(
            e.effective_power(bear),
            Some((5 + case.delta.0).max(0) as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bear),
            Some(expected_toughness),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&bear].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn q32_plus_minus_auras_cards_remove_the_modifier_when_the_aura_leaves() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_052_000 + index as u64);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        let aura = cast_aura(&mut e, case, bear);
        assert_eq!(
            e.effective_power(bear),
            Some((5 + case.delta.0).max(0) as u32),
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
        ensure_in_hand(&mut e, 0, "revoke_existence");
        let slot = hand_index_for_card(&e, 0, "revoke_existence");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(aura)))
            .expect("exile the Aura");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&aura].zone, Zone::Exile, "{}", case.id);
        assert_eq!(e.effective_power(bear), Some(5), "{}", case.id);
        assert_eq!(e.effective_toughness(bear), Some(5), "{}", case.id);
    }
}

#[test]
fn q32_plus_minus_auras_cards_reject_a_noncreature_and_fizzle_a_stale_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_053_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, "sky_diamond");
        give_mana(&mut e, 0, gift(case));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
                .is_err(),
            "{}: only a creature is legal",
            case.id
        );
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );

        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(case));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .expect("cast at the creature");
        e.apply_scenario_command(0, &pass()).expect("caster passes");
        give_mana(
            &mut e,
            1,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        ensure_in_hand(&mut e, 1, "swords_to_plowshares");
        let swords = hand_index_for_card(&e, 1, "swords_to_plowshares");
        e.apply_scenario_command(1, &cast_spell(swords, target_object(bear)))
            .expect("exile the target in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&bear].zone, Zone::Exile, "{}", case.id);
        let aura = e.state.players[0]
            .graveyard
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == case.id)
            .unwrap_or_else(|| panic!("{}: the fizzled Aura goes to the graveyard", case.id));
        assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
