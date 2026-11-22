//! Actual-card scenarios for the queue-v35 pump-and-shrink cohort.
//!
//! Expectations are printed Oracle values. CR 115.1/115.3 require two separate
//! target instances naming different creatures; CR 611.2a/514.2 end both pumps
//! at cleanup and CR 613.4c applies the positive and negative values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: (i32, i32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "consume_strength",
            mana: ManaGift {
                b: 1,
                g: 1,
                c: 1,
                ..Default::default()
            },
            amount: (2, 2),
        },
        Case {
            id: "leeching_bite",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            amount: (1, 1),
        },
        Case {
            id: "rites_of_reaping",
            mana: ManaGift {
                b: 1,
                g: 1,
                c: 4,
                ..Default::default()
            },
            amount: (3, 3),
        },
        Case {
            id: "schismotivate",
            mana: ManaGift {
                u: 1,
                r: 1,
                c: 1,
                ..Default::default()
            },
            amount: (4, 0),
        },
        Case {
            id: "steal_strength",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            amount: (1, 1),
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn cast_at(e: &mut GameEngine, case: &Case, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    let p = &e.state.players[0].mana_pool;
    assert_eq!(
        (p.white, p.blue, p.black, p.red, p.green, p.colorless),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn group_one(oid: u32) -> TargetRef {
    let mut target = target_object(oid).remove(0);
    target.group_index = 1;
    target
}

fn pair(first: u32, second: u32) -> Vec<TargetRef> {
    vec![target_object(first)[0], group_one(second)]
}

#[test]
fn q35_double_pump_shrink_canary_simplest() {
    let case = &cases()[4];
    let mut e = engine(35_090_001);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
    let second = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_at(&mut e, case, pair(first, second));
    assert_eq!(
        (e.effective_power(first), e.effective_toughness(first)),
        (Some(6), Some(6)),
        "Steal Strength pumps the first target"
    );
    assert_eq!(
        (e.effective_power(second), e.effective_toughness(second)),
        (Some(4), Some(4)),
        "Steal Strength shrinks the second target"
    );
}

#[test]
fn q35_double_pump_shrink_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(35_090_002);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
    let second = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_at(&mut e, case, pair(first, second));
    assert_eq!(
        (e.effective_power(first), e.effective_toughness(first)),
        (Some(8), Some(8)),
        "Rites of Reaping pumps the first target"
    );
    assert_eq!(
        (e.effective_power(second), e.effective_toughness(second)),
        (Some(2), Some(2)),
        "Rites of Reaping shrinks the second target"
    );
}

#[test]
fn q35_double_pump_shrink_cards_apply_the_printed_pump_and_shrink() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_091_000 + index as u64);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        let second = inject_creature_with_stats(&mut e, 1, "savannah_lions", 5, 5);
        let bystander = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_at(&mut e, case, pair(first, second));
        assert_eq!(
            (e.effective_power(first), e.effective_toughness(first)),
            (
                Some((5 + case.amount.0) as u32),
                Some((5 + case.amount.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert_eq!(
            (e.effective_power(second), e.effective_toughness(second)),
            (
                Some((5 - case.amount.0) as u32),
                Some((5 - case.amount.1) as u32)
            ),
            "{}: printed shrink",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: an untargeted creature is unaffected",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(first), e.effective_toughness(first)),
            (Some(5), Some(5)),
            "{}: the pump ends with the turn",
            case.id
        );
    }
}

#[test]
fn q35_double_pump_shrink_cards_require_two_distinct_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_092_000 + index as u64);
        let only = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(only)))
                .is_err(),
            "{}: one target cannot fill two required slots",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, pair(only, only)))
                .is_err(),
            "{}: the same creature cannot fill both target slots",
            case.id
        );
    }
}

#[test]
fn q35_double_pump_shrink_cards_keep_the_legal_target_when_the_other_leaves() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_093_000 + index as u64);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        let second = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, pair(first, second)))
            .expect("cast at both creatures");
        move_to_graveyard(&mut e, 1, second);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(first), e.effective_toughness(first)),
            (
                Some((5 + case.amount.0) as u32),
                Some((5 + case.amount.1) as u32)
            ),
            "{}: the remaining legal target still resolves",
            case.id
        );
        assert_eq!(
            e.state.objects[&second].zone,
            Zone::Graveyard,
            "{}: the departed target stays in the graveyard",
            case.id
        );
    }
}
