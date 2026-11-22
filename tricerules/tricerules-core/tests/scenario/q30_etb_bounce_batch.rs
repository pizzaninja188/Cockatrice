//! Actual-card scenarios for the queue-v30 enters-bounce cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the enters trigger;
//! CR 115.1/115.3 choose any creature; CR 608.2b fizzles a stale target. The two
//! "you may" cards also exercise the optional-ability decline path.
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
    may: bool,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "man-o-war",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            may: false,
        },
        Case {
            id: "aether_adept",
            mana: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            may: false,
        },
        Case {
            id: "voidwielder",
            mana: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
            body: (1, 4),
            may: true,
        },
        Case {
            id: "separatist_voidmage",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
            may: true,
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
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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
    assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
}

fn accept_if_pending(e: &mut GameEngine) {
    if e.state.pending_resolution.is_some() {
        answer_optional_triggered_ability_choice(e, ResolutionChoiceDecision::SelectBranch);
    }
}

#[test]
fn q30_etb_bounce_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(30_080_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, case);
    e.apply_scenario_command(0, &choose(target))
        .expect("choose creature");
    resolve_entire_stack_two_player(&mut e);
    accept_if_pending(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Hand);
}

#[test]
fn q30_etb_bounce_canary_hardest() {
    let case = &cases()[3];
    let mut e = engine(30_080_002);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, case);
    e.apply_scenario_command(0, &choose(target))
        .expect("choose creature");
    resolve_entire_stack_two_player(&mut e);
    accept_if_pending(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Hand);
}

#[test]
fn q30_etb_bounce_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_081_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast(&mut e, &case);
        e.apply_scenario_command(0, &choose(target))
            .expect("choose target");
        resolve_entire_stack_two_player(&mut e);
        accept_if_pending(&mut e);
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
fn q30_etb_bounce_cards_return_target_creature_to_hand() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_082_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast(&mut e, &case);
        e.apply_scenario_command(0, &choose(target))
            .expect("choose target");
        resolve_entire_stack_two_player(&mut e);
        accept_if_pending(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.players[1].hand.contains(&target), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_etb_bounce_cards_may_decline_where_printed() {
    for (index, case) in cases().into_iter().enumerate().filter(|(_, case)| case.may) {
        let mut e = engine(30_083_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
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
fn q30_etb_bounce_cards_accept_their_own_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_084_000 + index as u64);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast(&mut e, &case);
        let hand_before = e.state.players[0].hand.len();
        e.apply_scenario_command(0, &choose(own))
            .expect("choose own creature");
        resolve_entire_stack_two_player(&mut e);
        accept_if_pending(&mut e);
        assert_eq!(e.state.objects[&own].zone, Zone::Hand, "{}", case.id);
        assert!(
            e.state.players[0].hand.contains(&own),
            "{}: owner's hand",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}",
            case.id
        );
    }
}

#[test]
fn q30_etb_bounce_self_target_and_only_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_085_000 + index as u64);
        cast(&mut e, &case);
        let source = battlefield_object_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &choose(source))
            .expect("target the entering creature");
        resolve_entire_stack_two_player(&mut e);
        if case.may {
            assert!(
                e.state.pending_resolution.is_some(),
                "{}: choose to decline",
                case.id
            );
            answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
            assert_eq!(
                e.state.objects[&source].zone,
                Zone::Battlefield,
                "{}",
                case.id
            );
        } else {
            assert_eq!(e.state.objects[&source].zone, Zone::Hand, "{}", case.id);
            assert!(e.state.players[0].hand.contains(&source), "{}", case.id);
        }
    }
}

#[test]
fn q30_etb_bounce_cards_fizzle_on_stale_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_086_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast(&mut e, &case);
        e.apply_scenario_command(0, &choose(target))
            .expect("choose target");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("creature").zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        if e.state.pending_resolution.is_some() {
            answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
        }
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
