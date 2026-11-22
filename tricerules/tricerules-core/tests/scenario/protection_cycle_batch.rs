//! Actual-card scenarios for the reviewed protection keyword cycle.
//!
//! Expectations are printed Oracle values. CR 702.16b-f define protection as a
//! static ability with five separate effects; targeting by a source of the
//! protected quality is illegal and damage from such a source is prevented.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::{ProtectionCardType, ProtectionQuality};
use tricerules_cards::Color;

struct Case {
    id: &'static str,
    mana: &'static str,
    body: (u32, u32),
    quality: ProtectionQuality,
    attacker: &'static str,
    targeted_spell: &'static str,
    targeted_mana: ManaGift,
}

const fn gift(w: u32, u: u32, b: u32, r: u32, g: u32, c: u32) -> ManaGift {
    ManaGift { w, u, b, r, g, c }
}

fn cases() -> [Case; 20] {
    [
        Case {
            id: "galinas_knight",
            mana: "{W}{U}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::Red),
            attacker: "viashino_pyromancer",
            targeted_spell: "lightning_bolt",
            targeted_mana: gift(0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "oraxid",
            mana: "{3}{U}",
            body: (2, 3),
            quality: ProtectionQuality::Color(Color::Red),
            attacker: "viashino_pyromancer",
            targeted_spell: "lightning_bolt",
            targeted_mana: gift(0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "repentant_blacksmith",
            mana: "{1}{W}",
            body: (1, 2),
            quality: ProtectionQuality::Color(Color::Red),
            attacker: "viashino_pyromancer",
            targeted_spell: "lightning_bolt",
            targeted_mana: gift(0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "vedalken_outlander",
            mana: "{W}{U}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::Red),
            attacker: "viashino_pyromancer",
            targeted_spell: "lightning_bolt",
            targeted_mana: gift(0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "vulshok_refugee",
            mana: "{1}{R}{R}",
            body: (3, 2),
            quality: ProtectionQuality::Color(Color::Red),
            attacker: "viashino_pyromancer",
            targeted_spell: "lightning_bolt",
            targeted_mana: gift(0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "guma",
            mana: "{2}{R}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::Blue),
            attacker: "scholar_of_stars",
            targeted_spell: "unsummon",
            targeted_mana: gift(0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "karoo_meerkat",
            mana: "{1}{G}",
            body: (2, 1),
            quality: ProtectionQuality::Color(Color::Blue),
            attacker: "scholar_of_stars",
            targeted_spell: "unsummon",
            targeted_mana: gift(0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "nacatl_outlander",
            mana: "{R}{G}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::Blue),
            attacker: "scholar_of_stars",
            targeted_spell: "unsummon",
            targeted_mana: gift(0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "yavimaya_barbarian",
            mana: "{R}{G}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::Blue),
            attacker: "scholar_of_stars",
            targeted_spell: "unsummon",
            targeted_mana: gift(0, 1, 0, 0, 0, 0),
        },
        Case {
            id: "death_speakers",
            mana: "{W}",
            body: (1, 1),
            quality: ProtectionQuality::Color(Color::Black),
            attacker: "sorcerer_of_the_fang",
            targeted_spell: "murder",
            targeted_mana: gift(0, 0, 2, 0, 0, 1),
        },
        Case {
            id: "llanowar_knight",
            mana: "{G}{W}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::Black),
            attacker: "sorcerer_of_the_fang",
            targeted_spell: "murder",
            targeted_mana: gift(0, 0, 2, 0, 0, 1),
        },
        Case {
            id: "scalebanes_elite",
            mana: "{3}{G}{W}",
            body: (4, 4),
            quality: ProtectionQuality::Color(Color::Black),
            attacker: "sorcerer_of_the_fang",
            targeted_spell: "murder",
            targeted_mana: gift(0, 0, 2, 0, 0, 1),
        },
        Case {
            id: "valeron_outlander",
            mana: "{G}{W}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::Black),
            attacker: "sorcerer_of_the_fang",
            targeted_spell: "murder",
            targeted_mana: gift(0, 0, 2, 0, 0, 1),
        },
        Case {
            id: "goblin_outlander",
            mana: "{B}{R}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::White),
            attacker: "savannah_lions",
            targeted_spell: "swords_to_plowshares",
            targeted_mana: gift(1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "ihsans_shade",
            mana: "{3}{B}{B}{B}",
            body: (5, 5),
            quality: ProtectionQuality::Color(Color::White),
            attacker: "savannah_lions",
            targeted_spell: "swords_to_plowshares",
            targeted_mana: gift(1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "shivan_zombie",
            mana: "{B}{R}",
            body: (2, 2),
            quality: ProtectionQuality::Color(Color::White),
            attacker: "savannah_lions",
            targeted_spell: "swords_to_plowshares",
            targeted_mana: gift(1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "nacatl_savage",
            mana: "{1}{G}",
            body: (2, 1),
            quality: ProtectionQuality::CardType(ProtectionCardType::Artifact),
            attacker: "viseling",
            targeted_spell: "bonesplitter",
            targeted_mana: gift(0, 0, 0, 0, 0, 1),
        },
        Case {
            id: "tel-jilad_chosen",
            mana: "{1}{G}",
            body: (2, 1),
            quality: ProtectionQuality::CardType(ProtectionCardType::Artifact),
            attacker: "viseling",
            targeted_spell: "bonesplitter",
            targeted_mana: gift(0, 0, 0, 0, 0, 1),
        },
        Case {
            id: "tel-jilad_outrider",
            mana: "{3}{G}",
            body: (3, 1),
            quality: ProtectionQuality::CardType(ProtectionCardType::Artifact),
            attacker: "viseling",
            targeted_spell: "bonesplitter",
            targeted_mana: gift(0, 0, 0, 0, 0, 1),
        },
        Case {
            id: "yavimaya_scion",
            mana: "{4}{G}",
            body: (4, 4),
            quality: ProtectionQuality::CardType(ProtectionCardType::Artifact),
            attacker: "viseling",
            targeted_spell: "bonesplitter",
            targeted_mana: gift(0, 0, 0, 0, 0, 1),
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
fn protection_cycle_batch_cards_cast_with_exact_printed_characteristics_and_protection_quality() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_120_000 + index as u64);
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
            [case.quality],
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
fn protection_cycle_batch_cards_reject_targeting_by_the_protected_quality() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_121_000 + index as u64);
        if case.targeted_spell == "bonesplitter" {
            let protected = inject_creature_on_battlefield(&mut e, 0, case.id);
            let equipment = inject_permanent_on_battlefield(&mut e, 0, "bonesplitter");
            give_mana(&mut e, 0, case.targeted_mana);
            assert!(
                apply_ability(&mut e, 0, equipment, 0, target_object(protected)).is_err(),
                "{}: an artifact source cannot target the protected creature",
                case.id
            );
            assert!(e.state.stack.is_empty(), "{}", case.id);
            continue;
        }
        let protected = inject_creature_on_battlefield(&mut e, 1, case.id);
        inject_card_into_hand(&mut e, 0, case.targeted_spell);
        give_mana(&mut e, 0, case.targeted_mana);
        let slot = hand_index_for_card(&e, 0, case.targeted_spell);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(protected)))
                .is_err(),
            "{}: a source of the protected quality cannot target it",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            e.state.objects[&protected].zone,
            tricerules_core::Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn protection_cycle_batch_cards_prevent_damage_from_the_protected_quality() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_122_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.attacker);
        let attacker = put_creature_on_battlefield(&mut e, 0, case.attacker);
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
            tricerules_core::Zone::Battlefield,
            "{}: the protected creature survives the combat damage step",
            case.id
        );
        assert_eq!(
            e.state.objects[&protected].damage, 0,
            "{}: damage from the protected quality is prevented",
            case.id
        );
    }
}

#[test]
fn protection_cycle_batch_cards_cannot_be_blocked_by_the_protected_quality() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_123_000 + index as u64);
        let protected = inject_creature_on_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, case.attacker);
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
            "{}: a source of the protected quality cannot block it",
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
fn protection_cycle_batch_control_unprotected_blocker_takes_damage() {
    let mut e = engine(9_124_000);
    inject_card_into_hand(&mut e, 0, "savannah_lions");
    let attacker = put_creature_on_battlefield(&mut e, 0, "savannah_lions");
    inject_card_into_hand(&mut e, 1, "wall_of_stone");
    let blocker = put_creature_on_battlefield(&mut e, 1, "wall_of_stone");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(&mut e);
    e.apply_scenario_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: blocker,
        }]),
    )
    .expect("the ordinary blocker blocks");
    e.apply_scenario_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass to combat damage");
    assert_eq!(
        e.state.objects[&blocker].zone,
        tricerules_core::Zone::Battlefield,
        "the 0/8 blocker survives"
    );
    assert_eq!(
        e.state.objects[&blocker].damage, 2,
        "an unprotected blocker takes the attacker's damage"
    );
}
