//! Actual-card scenarios for the reviewed Swampwalk creature family.
//!
//! Expectations are printed Oracle values. CR 702.14a-c define Swampwalk as an
//! evasion ability: the creature can't be blocked while the defending player
//! controls a Swamp. CR 509.1b requires every applicable blocking restriction.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 21] {
    [
        Case {
            id: "anaconda",
            cast: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "anurid_murkdiver",
            cast: ManaGift {
                b: 2,
                c: 4,
                ..Default::default()
            },
            power: 4,
            toughness: 3,
        },
        Case {
            id: "bog_raiders",
            cast: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "bog_smugglers",
            cast: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "bog_tatters",
            cast: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            power: 4,
            toughness: 2,
        },
        Case {
            id: "bog_wraith",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "farbog_explorer",
            cast: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 3,
        },
        Case {
            id: "lost_soul",
            cast: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
        Case {
            id: "marsh_boa",
            cast: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "marsh_goblins",
            cast: ManaGift {
                b: 1,
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "marsh_threader",
            cast: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
        Case {
            id: "moor_fiend",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "plague_beetle",
            cast: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "pygmy_allosaurus",
            cast: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "raiding_nightstalker",
            cast: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "warthog",
            cast: ManaGift {
                g: 2,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 2,
        },
        Case {
            id: "wild_ox",
            cast: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "zodiac_ox",
            cast: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "zodiac_pig",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "zodiac_rat",
            cast: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "zodiac_snake",
            cast: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let mut e = game_start_engine(seed);
    advance_to_main1_from_game_start(&mut e);
    e
}

fn game_start_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn cast_creature(e: &mut GameEngine, id: &str, mana: ManaGift) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {id}: {error}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{id}: resolves"
    );
    slot
}

fn advance_landwalk_attack_to_manual_blocks(e: &mut GameEngine, landwalker_id: &str) -> (u32, u32) {
    advance_to_declare_attackers(e);
    let landwalker = inject_creature_on_battlefield(e, 0, landwalker_id);
    let vanilla_attacker = inject_creature_on_battlefield(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![landwalker, vanilla_attacker]))
        .expect("declare landwalker and vanilla attacker");
    e.apply_scenario_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass reaches manual blockers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    (landwalker, vanilla_attacker)
}

#[test]
fn swampwalk_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(771_000 + index as u64);
        let creature = cast_creature(&mut e, case.id, case.cast);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power),
            "{} power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{} toughness",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].owner, e.state.players[0].id,
            "{} owner",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].controller, e.state.players[0].id,
            "{} controller",
            case.id
        );
    }
}

#[test]
fn swampwalk_batch_cards_cannot_be_blocked_while_defender_controls_a_swamp() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = game_start_engine(772_000 + index as u64);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_permanent_on_battlefield(&mut e, 1, "swamp");
        let (attacker, _vanilla) = advance_landwalk_attack_to_manual_blocks(&mut e, case.id);
        let err = e
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker,
                }]),
            )
            .expect_err("Swampwalk attacker must reject a block while defender controls a Swamp");
        assert!(
            err.to_string().contains("evasion"),
            "{}: unexpected error: {err}",
            case.id
        );
        assert!(
            !e.state.combat.as_ref().expect("combat").blockers_declared,
            "{}: rejected block",
            case.id
        );
    }
}

#[test]
fn swampwalk_batch_cards_can_be_blocked_without_a_swamp() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = game_start_engine(773_000 + index as u64);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_permanent_on_battlefield(&mut e, 1, "forest");
        let (attacker, _vanilla) = advance_landwalk_attack_to_manual_blocks(&mut e, case.id);
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .unwrap_or_else(|error| panic!("{}: legal block rejected: {error}", case.id));
        assert!(
            e.state.combat.as_ref().expect("combat").blockers_declared,
            "{}: block accepted",
            case.id
        );
    }
}
