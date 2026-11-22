//! Actual-card scenarios for the reviewed block-pump cohort.
//!
//! Expectations are printed Oracle values. CR 509.3a applies the printed once-per-block wording; CR 509.3b describes the typed per-edge occurrence for the
//! attacker-blocker edge; CR 611.2a and 514.2 give the pump its until-end-of-turn
//! duration; CR 613.4c applies the layer 7c self pump.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::BlockPair;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    pump: (u32, u32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "folk_of_an-havva",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            body: (1, 1),
            pump: (2, 0),
        },
        Case {
            id: "giant_badger",
            mana: ManaGift {
                g: 2,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            pump: (2, 2),
        },
        Case {
            id: "royal_trooper",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            pump: (2, 2),
        },
        Case {
            id: "shu_defender",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            pump: (0, 2),
        },
        Case {
            id: "town_sentry",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            pump: (0, 2),
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "folk_of_an-havva" | "giant_badger"))
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

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
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
    let source = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1)),
        "{}: printed body",
        case.id
    );
    source
}

fn setup_block(case: &Case, seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("swamp", &[case.id]),
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
    let attacker = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = move_ready_to_battlefield(&mut e, 1, case.id);
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
    .expect("declare the blocker");
    (e, blocker, attacker)
}

fn setup_attack(case: &Case, seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("forest", &[case.id]),
        deck_with("swamp", &["grizzly_bears"]),
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
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    let blocker = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
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
    .expect("declare the blocker");
    (e, attacker)
}

#[test]
fn blocks_pump_batch_canary_cards_cast_with_exact_cost_and_body() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(900_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(
            !e.state.objects[&source].tapped,
            "{}: enters untapped",
            case.id
        );
    }
}

#[test]
fn blocks_pump_batch_canary_cards_pump_when_blocking() {
    for (index, case) in canaries().into_iter().enumerate() {
        let (mut e, blocker, attacker) = setup_block(&case, 901_000 + index as u64);
        assert_eq!(e.state.stack.len(), 1, "{}: one block trigger", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(blocker),
            Some(case.body.0 + case.pump.0),
            "{}: printed pump power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(blocker),
            Some(case.body.1 + case.pump.1),
            "{}: printed pump toughness",
            case.id
        );
        assert_eq!(
            (e.effective_power(attacker), e.effective_toughness(attacker)),
            (Some(2), Some(2)),
            "{}: the pump affects only the blocking source",
            case.id
        );
    }
}

#[test]
fn blocks_pump_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(902_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}",
            case.id
        );
        assert!(
            !e.effective_has_keyword(source, tricerules_cards::Keyword::Flying),
            "{}: no flying",
            case.id
        );
    }
}

#[test]
fn blocks_pump_batch_cards_pump_when_blocking_every_printed_card() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, blocker, _) = setup_block(&case, 903_000 + index as u64);
        assert_eq!(e.state.stack.len(), 1, "{}: exactly one trigger", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(blocker),
            Some(case.body.0 + case.pump.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(blocker),
            Some(case.body.1 + case.pump.1),
            "{}",
            case.id
        );
    }
}

#[test]
fn blocks_pump_batch_cards_do_not_pump_when_attacking_and_blocked() {
    for (index, case) in cases().into_iter().enumerate() {
        let (e, attacker) = setup_attack(&case, 904_000 + index as u64);
        assert!(
            e.state.stack.is_empty(),
            "{}: being blocked is not the printed blocking trigger",
            case.id
        );
        assert_eq!(
            e.effective_power(attacker),
            Some(case.body.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(attacker),
            Some(case.body.1),
            "{}",
            case.id
        );
    }
}
