//! Actual-card scenarios for the queue-v30 "counter target spell, then draw" cohort.
//!
//! Expectations are printed Oracle values. CR 701.6 counters the spell; CR 121 draws
//! the card only when the spell resolves; CR 608.2b removes a spell whose sole target
//! became illegal, so an illegal target prevents the draw.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    creature_only: bool,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "dismiss",
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            creature_only: false,
        },
        Case {
            id: "contradict",
            mana: ManaGift {
                u: 2,
                c: 3,
                ..Default::default()
            },
            creature_only: false,
        },
        Case {
            id: "bone_to_ash",
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            creature_only: true,
        },
        Case {
            id: "exclude",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            creature_only: true,
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
    let decks = Some(vec![
        deck_with("island", &["lightning_bolt", "grizzly_bears", "divination"]),
        deck_with("island", &[]),
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

fn top_card(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn cast_target(e: &mut GameEngine, case: &Case) -> u32 {
    let (card, mana, targets) = if case.creature_only {
        (
            "grizzly_bears",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            vec![],
        )
    } else {
        (
            "lightning_bolt",
            ManaGift {
                r: 1,
                ..Default::default()
            },
            target_player(1),
        )
    };
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .expect("cast the target spell");
    e.state.stack.last().expect("target spell").id
}

fn cast_counter(e: &mut GameEngine, case: &Case, target: u32) -> Result<(), String> {
    inject_card_into_hand(e, 1, case.id);
    give_mana(e, 1, gift(&case.mana));
    let slot = hand_index_for_card(e, 1, case.id);
    e.apply_scenario_command(1, &cast_spell(slot, target_object(target)))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn q30_counter_draw_canary_simplest() {
    let case = &cases()[3];
    let mut e = engine(30_010_001);
    let drawn = top_card(&mut e, 1, "forest");
    let target = cast_target(&mut e, case);
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    cast_counter(&mut e, case, target).expect("cast Exclude");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    assert!(e.state.players[1].hand.contains(&drawn));
}

#[test]
fn q30_counter_draw_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(30_010_002);
    let drawn = top_card(&mut e, 1, "forest");
    let target = cast_target(&mut e, case);
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    cast_counter(&mut e, case, target).expect("cast Contradict");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
}

#[test]
fn q30_counter_draw_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_011_000 + index as u64);
        let target = cast_target(&mut e, &case);
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        cast_counter(&mut e, &case, target).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        let pool = &e.state.players[1].mana_pool;
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
            "{}: exact printed cost",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
    }
}

#[test]
fn q30_counter_draw_cards_counter_and_draw() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_012_000 + index as u64);
        let drawn = top_card(&mut e, 1, "forest");
        let target = cast_target(&mut e, &case);
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        cast_counter(&mut e, &case, target).expect("counter the spell");
        let before = e.state.players[1].hand.len();
        let opponent_before = e.state.players[0].hand.len();
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: countered spell",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&target),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&drawn].zone,
            Zone::Hand,
            "{}: the counter's controller draws one",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            before + 1,
            "{}: exactly one card",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            opponent_before,
            "{}: the countered spell's controller's hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: the Lightning Bolt never resolved",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_counter_draw_cards_counter_a_creature_spell_too() {
    for (index, case) in cases()
        .into_iter()
        .enumerate()
        .filter(|(_, case)| !case.creature_only)
    {
        let mut e = engine(30_015_000 + index as u64);
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
            .expect("cast a creature spell");
        let target = e.state.stack.last().expect("spell").id;
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        cast_counter(&mut e, &case, target).expect("counter the creature spell");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            !e.state.players[0]
                .battlefield
                .iter()
                .any(|oid| e.state.objects[oid].card_id == "grizzly_bears"),
            "{}: the creature spell never entered",
            case.id
        );
    }
}

#[test]
fn q30_counter_draw_cards_reject_wrong_spell_type() {
    for (index, case) in cases()
        .into_iter()
        .enumerate()
        .filter(|(_, case)| case.creature_only)
    {
        let mut e = engine(30_013_000 + index as u64);
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
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .expect("cast the wrong spell type");
        let target = e.state.stack.last().expect("spell").id;
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        assert!(
            cast_counter(&mut e, &case, target).is_err(),
            "{}: a noncreature spell is not a legal target",
            case.id
        );
        assert_eq!(e.state.stack.len(), 1, "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 17,
            "{}: the bolt resolved",
            case.id
        );
    }
}

#[test]
fn q30_counter_draw_cards_fizzle_without_drawing() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_014_000 + index as u64);
        let drawn = top_card(&mut e, 1, "forest");
        let target = cast_target(&mut e, &case);
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        cast_counter(&mut e, &case, target).expect("counter the spell");
        e.state.stack.retain(|entry| entry.id != target);
        e.state.objects.get_mut(&target).expect("spell").zone = Zone::Graveyard;
        e.state.players[0].graveyard.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&drawn].zone,
            Zone::Library,
            "{}: no draw when the counter fizzles",
            case.id
        );
        assert!(!e.state.players[1].hand.contains(&drawn), "{}", case.id);
        assert!(
            e.state.players[1]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the counter is in its owner's graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
