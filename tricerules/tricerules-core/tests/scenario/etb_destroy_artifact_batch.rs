//! Actual-card scenarios for the reviewed enters-destroy-artifact cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the enters trigger on
//! the stack; CR 603.3d and 115.1 choose one artifact target; CR 701.8 destroys
//! the permanent. Artifact creatures are artifacts and remain legal targets.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "manic_vandal",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "oxidda_scrapmelter",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "uktabi_orangutan",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "viridian_shaman",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "vithian_renegades",
            mana: ManaGift {
                r: 1,
                g: 1,
                c: 1,
                ..Default::default()
            },
        },
    ]
}

fn choose_trigger_target(object_id: u32, kind: TargetRefKind) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: kind as i32,
                ..Default::default()
            }],
        })),
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card]),
        deck_with("mountain", &[]),
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

fn cast_creature(e: &mut GameEngine, case: &Case) {
    ensure_in_hand(e, 0, case.id);
    give_mana(
        e,
        0,
        ManaGift {
            w: case.mana.w,
            u: case.mana.u,
            b: case.mana.b,
            r: case.mana.r,
            g: case.mana.g,
            c: case.mana.c,
        },
    );
    let slot = hand_index_for_card(e, 0, case.id);
    semantic::accepted(e, 0, &cast_spell(slot, vec![]));
    pass_both_players(e);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{}: entry trigger waits",
        case.id
    );
}

#[test]
fn etb_destroy_artifact_batch_cards_destroy_a_target_artifact_on_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_180_000 + index as u64, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        cast_creature(&mut e, &case);
        e.apply_scenario_command(
            0,
            &choose_trigger_target(artifact, TargetRefKind::Permanent),
        )
        .expect("choose the artifact");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: artifact destroyed",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&artifact),
            "{}: destroyed artifact is in its owner's graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn etb_destroy_artifact_batch_cards_allow_artifact_creature_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_181_000 + index as u64, case.id);
        let artifact_creature = inject_creature_on_battlefield(&mut e, 1, "juggernaut");
        cast_creature(&mut e, &case);
        e.apply_scenario_command(
            0,
            &choose_trigger_target(artifact_creature, TargetRefKind::Permanent),
        )
        .expect("an artifact creature is an artifact target");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&artifact_creature].zone,
            Zone::Graveyard,
            "{}: artifact creature destroyed",
            case.id
        );
    }
}

#[test]
fn etb_destroy_artifact_batch_cards_reject_nonartifact_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_182_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        cast_creature(&mut e, &case);
        assert!(
            e.apply_scenario_command(
                0,
                &choose_trigger_target(creature, TargetRefKind::Permanent)
            )
            .is_err(),
            "{}: a nonartifact creature is not a legal target",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        e.apply_scenario_command(
            0,
            &choose_trigger_target(artifact, TargetRefKind::Permanent),
        )
        .expect("the artifact is the legal target");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: the legal artifact target is destroyed",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn etb_destroy_artifact_batch_cards_fizzle_stale_artifact_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_183_000 + index as u64, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        cast_creature(&mut e, &case);
        e.apply_scenario_command(
            0,
            &choose_trigger_target(artifact, TargetRefKind::Permanent),
        )
        .expect("choose the artifact");
        e.state.players[1]
            .battlefield
            .retain(|oid| *oid != artifact);
        e.state.objects.get_mut(&artifact).expect("artifact").zone = Zone::Exile;
        e.state.players[1].exile.push(artifact);
        *e.state.zone_change_generation.entry(artifact).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Exile,
            "{}: the stale target is not destroyed",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn etb_destroy_artifact_batch_cards_require_a_target_when_only_the_controller_has_an_artifact() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_184_000 + index as u64, case.id);
        let own_artifact = inject_permanent_on_battlefield(&mut e, 0, "bonesplitter");
        cast_creature(&mut e, &case);
        let decline = RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                decline: true,
                selected_modes: Vec::new(),
                targets: Vec::new(),
            })),
        };
        assert!(
            e.apply_scenario_command(0, &decline).is_err(),
            "{}: the mandatory target cannot be declined",
            case.id
        );
        assert_eq!(
            e.state.pending_triggers.len(),
            1,
            "{}: the rejected decline leaves the trigger waiting",
            case.id
        );
        e.apply_scenario_command(
            0,
            &choose_trigger_target(own_artifact, TargetRefKind::Permanent),
        )
        .expect("the controller's own artifact is the required target");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&own_artifact].zone,
            Zone::Graveyard,
            "{}: the controller's own artifact is destroyed",
            case.id
        );
    }
}
