//! Actual-card scenarios for the queue-v77 03-sac-artifact-destroy cohort.
//!
//! Expectations are printed Oracle values. CR 602.2b and 701.21a make sacrificing the source an
//! activation cost paid before the ability resolves; CR 115.1/601.2c restrict the target to an
//! artifact and CR 608.2b rechecks it on resolution. The source can target any artifact.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "goblin_replica",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "torch_fiend",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (2, 1),
        },
        Case {
            id: "reckless_reveler",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (2, 1),
        },
        Case {
            id: "ruinous_gremlin",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 1),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("mountain", &[])]);
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

fn activate_at(e: &mut GameEngine, source: u32, target: u32) -> Result<(), String> {
    let command = activate_ability_for(e, source, 0, target_object(target));
    e.apply_scenario_command(0, &command)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn q77_sac_artifact_destroy_canary_simplest_torch_fiend_sacrifices_to_destroy_an_artifact() {
    let case = &cases()[1];
    let mut e = engine(77_300_001);
    let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1)),
        "printed body"
    );
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    give_mana(&mut e, 0, gift(&case.mana));

    activate_at(&mut e, source, artifact).expect("activate Torch Fiend");
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "the source is sacrificed as a cost"
    );
    assert_eq!(
        e.state.objects[&artifact].zone,
        Zone::Battlefield,
        "the artifact is destroyed only on resolution"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed {{R}} cost");
    assert_eq!(e.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q77_sac_artifact_destroy_canary_hardest_goblin_replica_uses_its_artifact_body_and_cost() {
    let case = &cases()[0];
    let mut e = engine(77_300_002);
    let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(2)),
        "the Replica is a 2/2 artifact creature"
    );
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "mind_stone");
    give_mana(&mut e, 0, gift(&case.mana));
    activate_at(&mut e, source, artifact).expect("activate Goblin Replica");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "exact printed {{3}}{{R}} cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
}

#[test]
fn q77_sac_artifact_destroy_cards_use_their_printed_costs_and_bodies() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(77_301_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "sol_ring");
        give_mana(&mut e, 0, gift(&case.mana));
        activate_at(&mut e, source, artifact)
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: the source is the sacrifice cost",
            case.id
        );
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed mana",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q77_sac_artifact_destroy_cards_reject_nonartifact_targets_and_underfunded_costs_atomically() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(77_302_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        assert!(
            activate_at(&mut e, source, creature).is_err(),
            "{}: a creature is not a legal artifact target",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: rejected activation sacrifices nothing",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected activation spends no mana",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }

    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(77_302_100 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        let underfunded = ManaGift {
            r: case.mana.r.saturating_sub(1),
            c: case.mana.c.saturating_sub(1),
            ..Default::default()
        };
        give_mana(&mut e, 0, underfunded);
        let funded = pool(&e);
        assert!(
            activate_at(&mut e, source, artifact).is_err(),
            "{}: an underfunded activation is rejected",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: no source sacrifice",
            case.id
        );
        assert_eq!(pool(&e), funded, "{}: no mana spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q77_sac_artifact_destroy_cards_fizzle_on_stale_target_but_keep_the_paid_sacrifice() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(77_303_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        give_mana(&mut e, 0, gift(&case.mana));
        activate_at(&mut e, source, artifact).expect("activate at the artifact");
        e.state.players[1]
            .battlefield
            .retain(|oid| *oid != artifact);
        e.state.objects.get_mut(&artifact).expect("artifact").zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(artifact);
        *e.state.zone_change_generation.entry(artifact).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: the sacrifice cost remains paid",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q77_sac_artifact_destroy_cards_can_destroy_the_controllers_own_artifact() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(77_304_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let own_artifact = inject_permanent_on_battlefield(&mut e, 0, "bonesplitter");
        give_mana(&mut e, 0, gift(&case.mana));
        activate_at(&mut e, source, own_artifact)
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&own_artifact].zone,
            Zone::Graveyard,
            "{}: any artifact is a legal target",
            case.id
        );
    }
}
