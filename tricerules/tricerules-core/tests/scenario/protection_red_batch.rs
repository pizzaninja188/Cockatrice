//! Actual-card scenarios for the reviewed protection-from-red composition family.
//!
//! Expectations are printed Oracle values. CR 702.16b-f define protection as a
//! static ability with five separate effects; targeting by a red source is
//! illegal and damage from a red source is prevented. CR 702.7, 702.8 and
//! 702.29 govern the printed first strike, flash and cycling clauses.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::Color;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

struct Case {
    id: &'static str,
    mana: &'static str,
    body: (u32, u32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "defender_of_law",
            mana: "{2}{W}",
            body: (2, 1),
        },
        Case {
            id: "disciple_of_law",
            mana: "{1}{W}",
            body: (1, 2),
        },
        Case {
            id: "narwhal",
            mana: "{2}{U}{U}",
            body: (2, 2),
        },
        Case {
            id: "silver_knight",
            mana: "{W}{W}",
            body: (2, 2),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
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

fn gift_for(cost: &str) -> ManaGift {
    let mut m = ManaGift::default();
    for symbol in cost.split('}') {
        let Some(inner) = symbol.split('{').nth(1) else {
            continue;
        };
        for color in inner.chars() {
            match color {
                'W' => m.w += 1,
                'U' => m.u += 1,
                'B' => m.b += 1,
                'R' => m.r += 1,
                'G' => m.g += 1,
                _ => {}
            }
        }
        if let Ok(count) = inner.parse::<u32>() {
            m.c += count;
        }
    }
    m
}

fn cast_creature(e: &mut GameEngine, case: &Case) -> u32 {
    let slot = inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift_for(case.mana));
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast the protection creature");
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

fn advance_to_combat(e: &mut GameEngine, attacker: u32) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(e);
}

#[test]
fn protection_red_batch_cards_cast_with_exact_printed_characteristics_and_protection_quality() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_550_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.body.1),
            "{}",
            case.id
        );
        assert_eq!(
            e.characteristics(creature)
                .expect("characteristics")
                .protections,
            [ProtectionQuality::Color(Color::Red)],
            "{}: printed protection",
            case.id
        );
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, creature)
                .iter()
                .any(|label| label.starts_with("Protection from")),
            "{}: the protection is published",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn protection_red_batch_cards_reject_targeting_by_red() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_551_000 + index as u64);
        let protected = inject_creature_on_battlefield(&mut e, 1, case.id);
        inject_card_into_hand(&mut e, 0, "lightning_bolt");
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "lightning_bolt");
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(protected)))
                .is_err(),
            "{}: a red source cannot target it",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            e.state.objects[&protected].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn protection_red_batch_cards_prevent_damage_from_red() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_552_000 + index as u64);
        inject_card_into_hand(&mut e, 0, "viashino_pyromancer");
        let attacker = put_creature_on_battlefield(&mut e, 0, "viashino_pyromancer");
        inject_card_into_hand(&mut e, 1, case.id);
        let protected = put_creature_on_battlefield(&mut e, 1, case.id);
        advance_to_combat(&mut e, attacker);
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: protected,
            }]),
        )
        .expect("the protected creature blocks");
        e.apply_scenario_command(0, &pass())
            .expect("active pass declare blockers");
        e.apply_scenario_command(1, &pass())
            .expect("defender pass to combat damage");
        assert_eq!(
            e.state.objects[&protected].zone,
            Zone::Battlefield,
            "{}: the protected creature survives",
            case.id
        );
        assert_eq!(
            e.state.objects[&protected].damage, 0,
            "{}: red damage is prevented",
            case.id
        );
    }
}

#[test]
fn protection_red_batch_cards_cannot_be_blocked_by_red() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_553_000 + index as u64);
        let protected = inject_creature_on_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "viashino_pyromancer");
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![protected]))
            .expect("declare the protected attacker");
        pass_both_players(&mut e);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == protected && pair.blocker_id == blocker),
            "{}: a red source cannot block it",
            case.id
        );
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: protected,
                    blocker_id: blocker,
                }]),
            )
            .is_err(),
            "{}: the engine rejects the protected block",
            case.id
        );
    }
}

#[test]
fn protection_red_batch_cards_with_flash_use_instant_timing() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        9_554_000,
        &[0, 1],
        20,
        Some(vec![
            deck_with("plains", &[]),
            deck_with("plains", &["defender_of_law", "grizzly_bears"]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e.apply_scenario_command(0, &pass())
        .expect("offer priority to the nonactive player");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        1,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand = hand_index_for_card(&e, 1, "grizzly_bears");
    assert!(
        e.apply_scenario_command(1, &cast_spell(hand, vec![]))
            .is_err(),
        "a non-Flash creature cannot be cast during the opponent's main phase"
    );
    assert!(e.state.stack.is_empty());

    ensure_in_hand(&mut e, 1, "defender_of_law");
    give_mana(
        &mut e,
        1,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand = hand_index_for_card(&e, 1, "defender_of_law");
    e.apply_scenario_command(1, &cast_spell(hand, vec![]))
        .expect("Flash allows the creature during the opponent's main phase");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1]
            .battlefield
            .iter()
            .filter(|oid| e.state.objects[oid].card_id == "defender_of_law")
            .count(),
        1,
        "the flashed creature resolved"
    );
}

#[test]
fn protection_red_batch_cards_with_cycling_cycle_for_two() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        9_556_000,
        &[0, 1],
        20,
        Some(vec![
            deck_with("plains", &["disciple_of_law", "grizzly_bears"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "disciple_of_law");
    let card = e.state.players[0].hand[hand_index_for_card(&e, 0, "disciple_of_law")];
    let drawn = inject_library_card(&mut e, 0, "grizzly_bears");
    e.state.players[0]
        .library
        .retain(|candidate| *candidate != drawn);
    e.state.players[0].library.push_front(drawn);
    let hand_before = e.state.players[0].hand.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: card,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&card)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    };
    e.apply_scenario_command(0, &command)
        .expect("activate Cycling {2}");
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Graveyard,
        "cycling discards itself"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&drawn].zone,
        Zone::Hand,
        "cycling draws one card"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "discard one, draw one"
    );
}

#[test]
fn protection_red_batch_cards_with_first_strike_deal_damage_first() {
    for (index, case) in cases().into_iter().enumerate() {
        if !matches!(case.id, "narwhal" | "silver_knight") {
            continue;
        }
        let decks = Some(vec![
            deck_with("island", &[case.id]),
            deck_with("forest", &[]),
        ]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            9_557_000 + index as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_declare_attackers(&mut e);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the first-strike attacker");
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::FirstStrikeDamage,
            "{}: first-strike step",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.body.0).expect("power fits"),
            "{}: first-strike damage",
            case.id
        );
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.body.0).expect("power fits"),
            "{}: no regular-step damage",
            case.id
        );
    }
}
