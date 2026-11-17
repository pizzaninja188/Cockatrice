//! Actual-card scenarios for the queue-v30 optional enters-destroy cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the enters trigger;
//! CR 603.3d/115.1d choose its target; CR 701.8 destroys it. The controller may
//! decline the optional ability (CR 603.5), and the other permanent type is not a
//! legal target.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    target: &'static str,
    other: &'static str,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "war_priest_of_thune",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            target: "dampening_pulse",
            other: "cumber_stone",
        },
        Case {
            id: "enlightened_ascetic",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            target: "dampening_pulse",
            other: "cumber_stone",
        },
        Case {
            id: "wild_celebrants",
            mana: ManaGift {
                r: 2,
                c: 3,
                ..Default::default()
            },
            body: (5, 3),
            target: "cumber_stone",
            other: "dampening_pulse",
        },
        Case {
            id: "batterhorn",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            body: (4, 3),
            target: "cumber_stone",
            other: "dampening_pulse",
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
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
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

fn choose(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn cast(e: &mut GameEngine, case: &Case) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
    pass_both_players(e);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{}: entry trigger",
        case.id
    );
}

#[test]
fn q30_etb_naturalize_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(30_070_001);
    let target = inject_permanent_on_battlefield(&mut e, 1, case.target);
    cast(&mut e, case);
    e.apply_scenario_command(0, &choose(target))
        .expect("choose enchantment");
    resolve_entire_stack_two_player(&mut e);
    if e.state.pending_resolution.is_some() {
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    }
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn q30_etb_naturalize_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(30_070_002);
    let target = inject_permanent_on_battlefield(&mut e, 1, case.target);
    cast(&mut e, case);
    e.apply_scenario_command(0, &choose(target))
        .expect("choose artifact");
    resolve_entire_stack_two_player(&mut e);
    if e.state.pending_resolution.is_some() {
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    }
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn q30_etb_naturalize_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_071_000 + index as u64);
        let target = inject_permanent_on_battlefield(&mut e, 1, case.target);
        cast(&mut e, &case);
        e.apply_scenario_command(0, &choose(target))
            .expect("choose target");
        resolve_entire_stack_two_player(&mut e);
        if e.state.pending_resolution.is_some() {
            answer_optional_triggered_ability_choice(
                &mut e,
                ResolutionChoiceDecision::SelectBranch,
            );
        }
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}",
            case.id
        );
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
            "{}: exact printed cost",
            case.id
        );
    }
}

#[test]
fn q30_etb_naturalize_cards_may_destroy_matching_permanent() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_072_000 + index as u64);
        let target = inject_permanent_on_battlefield(&mut e, 1, case.target);
        cast(&mut e, &case);
        e.apply_scenario_command(0, &choose(target))
            .expect("choose target");
        resolve_entire_stack_two_player(&mut e);
        if e.state.pending_resolution.is_some() {
            answer_optional_triggered_ability_choice(
                &mut e,
                ResolutionChoiceDecision::SelectBranch,
            );
        }
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&target),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_etb_naturalize_cards_may_decline() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_073_000 + index as u64);
        let target = inject_permanent_on_battlefield(&mut e, 1, case.target);
        cast(&mut e, &case);
        e.apply_scenario_command(0, &choose(target))
            .expect("choose target before declining");
        resolve_entire_stack_two_player(&mut e);
        if e.state.pending_resolution.is_some() {
            answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
        }
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_etb_naturalize_cards_ignore_the_other_permanent_type() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_074_000 + index as u64);
        let other = inject_permanent_on_battlefield(&mut e, 1, case.other);
        let legal = inject_permanent_on_battlefield(&mut e, 1, case.target);
        cast(&mut e, &case);
        assert!(
            e.apply_scenario_command(0, &choose(other)).is_err(),
            "{}: the other permanent type is not a legal target",
            case.id
        );
        assert_eq!(
            e.state.objects[&other].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        e.apply_scenario_command(0, &choose(legal))
            .expect("choose the legal target");
        resolve_entire_stack_two_player(&mut e);
        if e.state.pending_resolution.is_some() {
            answer_optional_triggered_ability_choice(
                &mut e,
                ResolutionChoiceDecision::SelectBranch,
            );
        }
        assert_eq!(e.state.objects[&legal].zone, Zone::Graveyard, "{}", case.id);
    }
}
