//! Actual-card scenarios for the queue-v32 self-sacrifice ping cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activate the ability;
//! CR 118.3/701.21a pay the sacrifice cost; CR 115.1 permits any target; and
//! CR 608.2b leaves a stale target without damage.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: u32,
    starting_life: i32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "mogg_fanatic",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            amount: 1,
            starting_life: 20,
        },
        Case {
            id: "moonglove_extract",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            amount: 2,
            starting_life: 20,
        },
        Case {
            id: "seal_of_fire",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            amount: 2,
            starting_life: 20,
        },
        Case {
            id: "shock_troops",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            amount: 2,
            starting_life: 20,
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
        deck_with("mountain", &[]),
        deck_with("forest", &["grizzly_bears", "swords_to_plowshares"]),
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

fn cast_ping(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(case));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let source = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    assert_eq!(e.state.players[0].life, case.starting_life, "{}", case.id);
    source
}

#[test]
fn q32_self_sac_ping_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(32_060_001);
    let source = cast_ping(&mut e, case);
    apply_ability(&mut e, 0, source, 0, target_player(1)).expect("activate at the opponent");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_self_sac_ping_canary_hardest() {
    let case = &cases()[3];
    let mut e = engine(32_060_002);
    let source = cast_ping(&mut e, case);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    apply_ability(&mut e, 0, source, 0, target_object(bear)).expect("activate at the bear");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn q32_self_sac_ping_cards_deal_the_printed_damage_and_sacrifice_themselves() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_061_000 + index as u64);
        let source = cast_ping(&mut e, case);
        apply_ability(&mut e, 0, source, 0, target_player(1))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - case.amount as i32,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: a sacrificed source cannot be activated again",
            case.id
        );
    }
}

#[test]
fn q32_self_sac_ping_cards_require_a_legal_target_before_sacrificing() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_062_000 + index as u64);
        let source = cast_ping(&mut e, case);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: any target is mandatory",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
    }
}

#[test]
fn q32_self_sac_ping_cards_do_not_damage_a_stale_target_but_still_sacrifice() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_063_000 + index as u64);
        let source = cast_ping(&mut e, case);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        apply_ability(&mut e, 0, source, 0, target_object(bear)).expect("activate at the bear");
        e.apply_scenario_command(0, &pass())
            .expect("activator passes");
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
        assert_eq!(e.state.objects[&bear].damage, 0, "{}", case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 22,
            "{}: Swords to Plowshares gains two life",
            case.id
        );
    }
}
