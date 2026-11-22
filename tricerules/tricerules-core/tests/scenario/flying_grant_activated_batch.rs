//! Actual-card scenarios for the reviewed activated flying-grant cohort.
//!
//! Expectations are printed Oracle values. CR 602.1-2 activate the printed mana
//! ability; CR 702.9a-b make flying an evasion ability; CR 611.2a applies the
//! grant until end of turn; CR 509.1b and 509.1h keep an existing block;
//! CR 514.2 ends it.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{EngineError, Zone};

struct Case {
    id: &'static str,
    cast: ManaGift,
    activation: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "cobalt_golem",
            cast: ManaGift {
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 3,
        },
        Case {
            id: "kor_sky_climber",
            cast: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            activation: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 2,
        },
        Case {
            id: "leaping_master",
            cast: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
        Case {
            id: "roofstalker_wight",
            cast: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
        Case {
            id: "steeple_creeper",
            cast: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            activation: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            power: 4,
            toughness: 2,
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "cobalt_golem" | "steeple_creeper"))
        .collect()
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn combat_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card]),
        deck_with("island", &["grizzly_bears"]),
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
    advance_to_declare_attackers(&mut e);
    e
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.cast));
    inject_card_into_hand(e, 0, case.id);
    let hand = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    let creature = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(case.power), Some(case.toughness)),
        "{}: printed body",
        case.id
    );
    assert!(
        !e.effective_has_keyword(creature, Keyword::Flying),
        "{}: no printed flying",
        case.id
    );
    creature
}

fn activate_flying(e: &mut GameEngine, creature: u32, case: &Case) {
    give_mana(e, 0, gift(&case.activation));
    apply_ability(e, 0, creature, 0, vec![])
        .unwrap_or_else(|error| panic!("{}: activate flying: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact activation cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn flying_grant_activated_batch_canary_cards_gain_flying_only_after_activation() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(910_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: ground creature before activation",
            case.id
        );
        activate_flying(&mut e, creature, &case);
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}: flying after activation",
            case.id
        );
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: the grant does not tap",
            case.id
        );
    }
}

#[test]
fn flying_grant_activated_batch_cards_lose_flying_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(911_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        activate_flying(&mut e, creature, &case);
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: CR 514.2 ends the grant",
            case.id
        );
    }
}

#[test]
fn flying_grant_activated_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(912_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn flying_grant_activated_batch_cards_gain_flying_for_the_exact_activation_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(913_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        activate_flying(&mut e, creature, &case);
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}",
            case.id
        );
        assert!(!e.state.objects[&creature].tapped, "{}", case.id);
    }
}

#[test]
fn flying_grant_activated_batch_cards_allow_a_ground_blocker_only_before_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut before = combat_engine(914_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut before, 0, case.id);
        let ground = inject_creature_on_battlefield(&mut before, 1, "grizzly_bears");
        before
            .apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the ground attacker");
        pass_both_players(&mut before);
        before
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: ground,
                }]),
            )
            .expect("a ground creature blocks before flying is granted");
        give_mana(&mut before, 0, gift(&case.activation));
        apply_ability(&mut before, 0, attacker, 0, vec![])
            .expect("activate flying after the block");
        resolve_entire_stack_two_player(&mut before);
        assert_eq!(
            before.state.combat.as_ref().expect("combat").blockers[&attacker],
            [ground],
            "{}: gaining flying after being blocked does not unblock",
            case.id
        );

        let mut after = combat_engine(915_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut after, 0, case.id);
        let ground = inject_creature_on_battlefield(&mut after, 1, "grizzly_bears");
        after
            .apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        give_mana(&mut after, 0, gift(&case.activation));
        apply_ability(&mut after, 0, attacker, 0, vec![]).expect("activate flying before blockers");
        resolve_entire_stack_two_player(&mut after);
        pass_both_players(&mut after);
        assert!(
            matches!(
                after.apply_scenario_command(
                    1,
                    &declare_blockers(vec![BlockPair {
                        attacker_id: attacker,
                        blocker_id: ground
                    }]),
                ),
                Err(EngineError::Illegal(_))
            ),
            "{}: a ground creature cannot block the flyer",
            case.id
        );
    }
}

#[test]
fn flying_grant_activated_batch_cards_reject_activation_without_the_exact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(916_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the grant needs its exact mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: the rejected activation grants nothing",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
