//! Actual-card scenarios for the reviewed Island landwalk creature family.
//!
//! Expectations are printed Oracle values. CR 702.14a-c define Island landwalk as an evasion ability: the creature can't be blocked while the defending player controls a Island. CR 509.1b requires every applicable blocking restriction to hold. The cards have no other abilities, costs, triggers or targets.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 9] {
    [
        Case {
            id: "benthic_behemoth",
            cast: ManaGift {
                u: 3,
                c: 5,
                ..Default::default()
            },
            power: 7,
            toughness: 6,
        },
        Case {
            id: "bull_hippo",
            cast: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "devouring_deep",
            cast: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 2,
        },
        Case {
            id: "grayscaled_gharial",
            cast: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "pale_bears",
            cast: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "river_bear",
            cast: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "rootwater_commando",
            cast: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "segovian_leviathan",
            cast: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "zodiac_horse",
            cast: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
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
    let pool = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "{id}: exact cost consumed"
    );
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
fn islandwalk_batch_cards_cast_with_exact_printed_characteristics() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(771_000 + case_index as u64);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
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
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{} opponent hand",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{} opponent library",
            case.id
        );
    }
}

#[test]
fn islandwalk_batch_cards_cannot_be_blocked_while_defender_controls_an_island() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = game_start_engine(772_000 + case_index as u64);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_permanent_on_battlefield(&mut e, 1, "island");
        let (attacker, _vanilla) = advance_landwalk_attack_to_manual_blocks(&mut e, case.id);
        let err = e
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker,
                }]),
            )
            .expect_err(
                "landwalk attacker must reject a block while defender controls its land type",
            );
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
fn islandwalk_batch_cards_can_be_blocked_without_an_island() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = game_start_engine(773_000 + case_index as u64);
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
        .expect("Island landwalk is inactive without its land type");
    }
}
