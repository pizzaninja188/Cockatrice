//! Actual-card scenarios for the reviewed enters-destroy-land cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the enters trigger on
//! the stack; CR 603.3d and 115.1d choose one land target; CR 701.8 destroys the
//! permanent; CR 608.2b leaves a stale target unaffected.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "goblin_settler",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "ogre_arsonist",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
        },
        Case {
            id: "ravaging_horde",
            mana: ManaGift {
                r: 2,
                c: 3,
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
        deck_with("mountain", &[card]),
        deck_with("forest", &[]),
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
fn etb_destroy_land_batch_cards_destroy_a_target_land_on_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_230_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        cast_creature(&mut e, &case);
        e.apply_scenario_command(0, &choose_trigger_target(land, TargetRefKind::Permanent))
            .expect("choose the land");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Graveyard,
            "{}: land destroyed",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&land),
            "{}: destroyed land is in its owner's graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn etb_destroy_land_batch_cards_allow_any_players_land() {
    for (index, case) in cases().into_iter().enumerate() {
        for (player, label) in [(0usize, "controller"), (1usize, "opponent")] {
            let mut e = engine(9_231_000 + index as u64 * 10 + player as u64, case.id);
            let land = inject_permanent_on_battlefield(&mut e, player, "forest");
            cast_creature(&mut e, &case);
            e.apply_scenario_command(0, &choose_trigger_target(land, TargetRefKind::Permanent))
                .unwrap_or_else(|error| panic!("{}: {label} land rejected: {error}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.state.objects[&land].zone,
                Zone::Graveyard,
                "{}: {label} land destroyed",
                case.id
            );
        }
    }
}

#[test]
fn etb_destroy_land_batch_cards_reject_nonland_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_232_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        cast_creature(&mut e, &case);
        assert!(
            e.apply_scenario_command(
                0,
                &choose_trigger_target(creature, TargetRefKind::Permanent)
            )
            .is_err(),
            "{}: a creature is not a land target",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        e.apply_scenario_command(0, &choose_trigger_target(land, TargetRefKind::Permanent))
            .expect("the land is the legal target");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Graveyard,
            "{}: the legal land target is destroyed",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn etb_destroy_land_batch_cards_fizzle_stale_land_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_233_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        cast_creature(&mut e, &case);
        e.apply_scenario_command(0, &choose_trigger_target(land, TargetRefKind::Permanent))
            .expect("choose the land");
        e.state.players[1].battlefield.retain(|oid| *oid != land);
        e.state.objects.get_mut(&land).expect("land").zone = Zone::Exile;
        e.state.players[1].exile.push(land);
        *e.state.zone_change_generation.entry(land).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Exile,
            "{}: the stale target is not destroyed",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn etb_destroy_land_batch_cards_require_a_target_when_only_the_controller_has_a_land() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_234_000 + index as u64, case.id);
        let own_land = inject_permanent_on_battlefield(&mut e, 0, "forest");
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
            &choose_trigger_target(own_land, TargetRefKind::Permanent),
        )
        .expect("the controller's own land is the required target");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&own_land].zone,
            Zone::Graveyard,
            "{}: the controller's own land is destroyed",
            case.id
        );
    }
}
