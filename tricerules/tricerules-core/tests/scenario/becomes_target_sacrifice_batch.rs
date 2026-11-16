//! Actual-card scenarios for the reviewed becomes-target sacrifice cohort.
//!
//! Expectations are printed Oracle values. CR 603.2 stages the trigger when the
//! creature becomes a target; CR 603.3 and 608.1 put that trigger on top and
//! resolve it before the targeting spell or ability; CR 701.21a sacrifices the source; CR 608.2b
//! counters a spell or ability whose only target has left.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "frost_walker",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            body: (4, 1),
        },
        Case {
            id: "phantasmal_bear",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "phantom_beast",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            body: (4, 5),
        },
        Case {
            id: "skulking_fugitive",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            body: (3, 4),
        },
        Case {
            id: "tar_pit_warrior",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            body: (3, 4),
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "phantasmal_bear" | "phantom_beast"))
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
    let decks = Some(vec![
        deck_with("forest", &[card, "lightning_bolt", "grizzly_bears"]),
        deck_with(
            "mountain",
            &["lightning_bolt", "sorins_thirst", "prodigal_sorcerer"],
        ),
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

fn resolve_top_stack(e: &mut GameEngine) {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_scenario_command(first, &pass())
        .expect("first pass");
    e.apply_scenario_command(second, &pass())
        .expect("second pass resolves the top stack item");
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
    let creature = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(case.body.0), Some(case.body.1)),
        "{}: printed body",
        case.id
    );
    creature
}

#[test]
fn becomes_target_sacrifice_batch_canary_cards_sacrifice_to_a_targeting_spell() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(920_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        ensure_in_hand(&mut e, 1, "sorins_thirst");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes priority to the opponent");
        give_mana(
            &mut e,
            1,
            ManaGift {
                b: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 1, "sorins_thirst");
        e.apply_scenario_command(1, &cast_spell(slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{}: drain the creature: {error:?}", case.id));
        assert_eq!(
            e.state.stack.len(),
            2,
            "{}: the sacrifice trigger is above the spell",
            case.id
        );
        resolve_top_stack(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{}: the trigger resolves and sacrifices the source",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&creature),
            "{}: the sacrificed creature is in its owner's graveyard",
            case.id
        );
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: the spell is still on the stack",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.players[1]
                .graveyard
                .iter()
                .any(|oid| { e.state.objects[oid].card_id == "sorins_thirst" }),
            "{}: the spell was countered by its only target leaving",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: the fizzled spell gains no life",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn becomes_target_sacrifice_batch_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(921_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}",
            case.id
        );
        assert!(!e.state.objects[&creature].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn becomes_target_sacrifice_batch_cards_sacrifice_to_a_targeting_ability() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(922_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        let pinger = move_ready_to_battlefield(&mut e, 1, "prodigal_sorcerer");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes priority to the opponent");
        apply_ability(&mut e, 1, pinger, 0, target_object(creature))
            .unwrap_or_else(|error| panic!("{}: activate the ping: {error:?}", case.id));
        assert_eq!(
            e.state.stack.len(),
            2,
            "{}: trigger above the ability",
            case.id
        );
        resolve_top_stack(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{}: an ability target also sacrifices the source",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&creature),
            "{}: the sacrificed creature is in its owner's graveyard",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.stack.len(), 0, "{}", case.id);
    }
}

#[test]
fn becomes_target_sacrifice_batch_cards_trigger_for_their_own_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(923_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        ensure_in_hand(&mut e, 0, "lightning_bolt");
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "lightning_bolt");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{}: bolt own creature: {error:?}", case.id));
        resolve_top_stack(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{}: Oracle has no opponent-only restriction",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&creature),
            "{}: the sacrificed creature is in its owner's graveyard",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.stack.len(), 0, "{}", case.id);
    }
}

#[test]
fn becomes_target_sacrifice_batch_cards_do_not_trigger_without_a_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(924_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        assert!(
            e.state.stack.is_empty(),
            "{}: casting the creature does not target it",
            case.id
        );
        ensure_in_hand(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast a non-targeting creature spell");
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: the non-targeting spell is on the stack",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.stack.is_empty(),
            "{}: no sacrifice trigger fires",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}: a non-targeting spell does not sacrifice the source",
            case.id
        );
    }
}
