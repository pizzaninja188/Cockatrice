//! Queue-v29 canaries: "Whenever this creature attacks, you may tap target creature."
//!
//! CR 508.3a/603.2c, 603.5, 115.1 and 701.26. Expectations are printed Oracle values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::TurnStep;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand, TargetRef,
    TargetRefKind,
};

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn case(id: &'static str) -> Case {
    match id {
        "seasoned_marshal" | "flanking_troops" | "alaborn_cavalier" => Case {
            id,
            power: 2,
            toughness: 2,
        },
        _ => unreachable!(),
    }
}

fn cases() -> [Case; 3] {
    [
        case("seasoned_marshal"),
        case("flanking_troops"),
        case("alaborn_cavalier"),
    ]
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card]),
        deck_with("forest", &["grizzly_bears"]),
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

fn advance_to_declare_attackers(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    for _ in 0..e.state.players.len() {
        if e.state.turn_step == TurnStep::DeclareAttackers {
            return;
        }
        let player = e.state.priority_player_id();
        e.apply_scenario_command(player, &pass())
            .expect("pass in beginning of combat");
    }
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
}

fn assert_canary(case: Case, seed: u64) {
    let mut e = engine(seed, case.id);
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    let blocker = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    assert_eq!(
        (e.effective_power(attacker), e.effective_toughness(attacker)),
        (Some(case.power), Some(case.toughness)),
        "{}: printed body",
        case.id
    );
    advance_to_declare_attackers(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
    e.apply_scenario_command(0, &choose_trigger_target(blocker))
        .expect("choose target creature");
    resolve_entire_stack_two_player(&mut e);
    if e.state.pending_resolution.is_some() {
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    }
    assert!(
        e.state.objects[&blocker].tapped,
        "{}: the chosen creature is tapped",
        case.id
    );
    assert!(
        e.state.objects[&attacker].tapped,
        "{}: attacking tapped the attacker",
        case.id
    );
}

#[test]
fn q29_attack_may_tap_canary_simplest() {
    assert_canary(case("seasoned_marshal"), 29_030_001);
}

#[test]
fn q29_attack_may_tap_canary_hardest() {
    assert_canary(case("alaborn_cavalier"), 29_030_002);
}

fn attack_with(e: &mut GameEngine, attacker: u32) {
    advance_to_declare_attackers(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    assert_eq!(e.state.pending_triggers.len(), 1);
}

fn accept_trigger(e: &mut GameEngine) {
    resolve_entire_stack_two_player(e);
    if e.state.pending_resolution.is_some() {
        answer_optional_triggered_ability_choice(e, ResolutionChoiceDecision::SelectBranch);
    }
}

#[test]
fn q29_attack_may_tap_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_031_000 + index as u64, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, case.id);
        let hand = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
    }
}

#[test]
fn q29_attack_may_tap_cards_may_decline_the_optional_trigger() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_032_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let blocker = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        attack_with(&mut e, attacker);
        e.apply_scenario_command(0, &choose_trigger_target(blocker))
            .expect("choose a target before declining");
        resolve_entire_stack_two_player(&mut e);
        if e.state.pending_resolution.is_some() {
            answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
        }
        assert!(
            !e.state.objects[&blocker].tapped,
            "{}: declining the may ability does not tap the target",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q29_attack_may_tap_cards_can_target_their_own_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_033_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        attack_with(&mut e, attacker);
        e.apply_scenario_command(0, &choose_trigger_target(own))
            .expect("target any creature, including your own");
        accept_trigger(&mut e);
        assert!(e.state.objects[&own].tapped, "{}", case.id);
    }
}

#[test]
fn q29_attack_may_tap_cards_accept_a_tapped_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_034_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let blocker = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&blocker).expect("blocker").tapped = true;
        attack_with(&mut e, attacker);
        e.apply_scenario_command(0, &choose_trigger_target(blocker))
            .expect("an already tapped creature is legal");
        accept_trigger(&mut e);
        assert!(e.state.objects[&blocker].tapped, "{}", case.id);
    }
}

#[test]
fn q29_attack_may_tap_cards_target_any_creature_in_a_three_player_game() {
    for (index, case) in cases().into_iter().enumerate() {
        let decks = Some(vec![
            deck_with("plains", &[case.id]),
            deck_with("forest", &["grizzly_bears"]),
            deck_with("forest", &["grizzly_bears"]),
        ]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            29_035_000 + index as u64,
            &[0, 1, 2],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let defender_creature = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        let bystander = move_ready_to_battlefield(&mut e, 2, "grizzly_bears");
        advance_to_declare_attackers(&mut e);
        let command = RuledCommand {
            cmd: Some(Cmd::DeclareAttackers(
                tricerules_proto::ruled::v1::DeclareAttackers {
                    assignments: vec![tricerules_proto::ruled::v1::AttackAssignment {
                        attacker_object_id: attacker,
                        attacker_zone_change_generation: e
                            .state
                            .zone_change_generation
                            .get(&attacker)
                            .copied()
                            .unwrap_or(0),
                        defender: Some(TargetRef {
                            object_id: e.state.players[1].id as u32,
                            kind: TargetRefKind::Player as i32,
                            ..Default::default()
                        }),
                        defending_player_id: e.state.players[1].id,
                        ..Default::default()
                    }],
                },
            )),
        };
        e.apply_scenario_command(0, &command)
            .expect("attack player 1");
        assert_eq!(e.state.pending_triggers.len(), 1);
        e.apply_scenario_command(0, &choose_trigger_target(bystander))
            .expect("target creature is not restricted to the defending player");
        while !e.state.stack.is_empty() {
            pass_priority_round(&mut e);
        }
        if e.state.pending_resolution.is_some() {
            answer_optional_triggered_ability_choice(
                &mut e,
                ResolutionChoiceDecision::SelectBranch,
            );
        }
        assert!(e.state.objects[&bystander].tapped, "{}", case.id);
        assert!(
            !e.state.objects[&defender_creature].tapped,
            "{}: the chosen bystander was the target",
            case.id
        );
    }
}
