//! Actual-card scenarios for the queue-v20 mana-plus-tap tapper cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 govern the activated
//! ability and its cost, CR 118.3 the mana payment, CR 701.26 tapping, CR 608.2b
//! resolution revalidation, and CR 302.6 the tap-cost summoning restriction.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    activation: ManaGift,
    power: Option<u32>,
    toughness: Option<u32>,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "akroan_jailer",
            activation: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            power: Some(1),
            toughness: Some(1),
        },
        Case {
            id: "blinding_souleater",
            activation: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: Some(1),
            toughness: Some(3),
        },
        Case {
            id: "checkpoint_officer",
            activation: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: Some(1),
            toughness: Some(2),
        },
        Case {
            id: "elite_arrester",
            activation: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            power: Some(0),
            toughness: Some(3),
        },
        Case {
            id: "rathi_trapper",
            activation: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: Some(1),
            toughness: Some(2),
        },
        Case {
            id: "tyrants_machine",
            activation: ManaGift {
                c: 4,
                ..Default::default()
            },
            power: None,
            toughness: None,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
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

#[test]
fn tap_target_mana_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_040_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        assert_eq!(e.effective_power(source), case.power, "{}", case.id);
        assert_eq!(e.effective_toughness(source), case.toughness, "{}", case.id);
        give_mana(&mut e, 0, gift(&case.activation));
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .expect("activate the tap ability");
        assert!(
            e.state.objects[&source].tapped,
            "{}: tapping the source is a cost",
            case.id
        );
        assert!(
            !e.state.objects[&target].tapped,
            "{}: the target is tapped only on resolution",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.objects[&target].tapped,
            "{}: the target creature is tapped",
            case.id
        );
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
fn tap_target_mana_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_041_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        give_mana(&mut e, 0, gift(&case.activation));
        let before = pool(&e);
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(artifact))
            )
            .is_err(),
            "{}: an artifact is not a creature target",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: rejected activation", case.id);
        assert!(
            !e.state.objects[&source].tapped,
            "{}: a rejected activation does not tap the source",
            case.id
        );
        assert!(
            !e.state.objects[&artifact].tapped,
            "{}: a rejected activation does not tap the artifact",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_target_mana_batch_cards_handle_stale_or_expired_state() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_042_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.activation));
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .expect("activate the tap ability");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        e.state.players[1].exile.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: a stale target is not returned",
            case.id
        );
        assert!(
            !e.state.objects[&target].tapped,
            "{}: a stale target is not tapped",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the tap cost remains paid",
            case.id
        );
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: the mana cost remains paid",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_target_mana_batch_cards_reject_insufficient_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_043_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(target)),
            )
            .is_err(),
            "{}: the activation needs its exact mana",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert!(!e.state.objects[&target].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        // CR 302.6 restricts only a creature's tap-symbol abilities; the
        // artifact Tyrant's Machine can tap even on the turn it entered.
        if case.power.is_some() {
            let mut sick = engine(1_043_500 + index as u64, case.id);
            let sick_source = move_ready_to_battlefield(&mut sick, 0, case.id);
            sick.state
                .objects
                .get_mut(&sick_source)
                .expect("source")
                .summoning_sick = true;
            let legal_target = inject_creature_on_battlefield(&mut sick, 1, "grizzly_bears");
            give_mana(&mut sick, 0, gift(&case.activation));
            assert!(
                sick.apply_scenario_command(
                    0,
                    &activate_ability_for(&sick, sick_source, 0, target_object(legal_target)),
                )
                .is_err(),
                "{}: a summoning-sick creature cannot pay a tap cost",
                case.id
            );
            assert!(!sick.state.objects[&sick_source].tapped, "{}", case.id);
            let activation = gift(&case.activation);
            assert_eq!(
                pool(&sick),
                (
                    activation.w,
                    activation.u,
                    activation.b,
                    activation.r,
                    activation.g,
                    activation.c
                ),
                "{}: no mana is spent",
                case.id
            );
            assert!(sick.state.stack.is_empty(), "{}", case.id);
        } else {
            // A noncreature artifact's tap ability is not restricted by CR 302.6.
            let mut artifact = engine(1_043_500 + index as u64, case.id);
            let artifact_source = move_ready_to_battlefield(&mut artifact, 0, case.id);
            artifact
                .state
                .objects
                .get_mut(&artifact_source)
                .expect("source")
                .summoning_sick = true;
            let legal_target = inject_creature_on_battlefield(&mut artifact, 1, "grizzly_bears");
            give_mana(&mut artifact, 0, gift(&case.activation));
            artifact
                .apply_scenario_command(
                    0,
                    &activate_ability_for(
                        &artifact,
                        artifact_source,
                        0,
                        target_object(legal_target),
                    ),
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{}: a noncreature artifact may tap on entry: {error:?}",
                        case.id
                    )
                });
            resolve_entire_stack_two_player(&mut artifact);
            assert!(
                artifact.state.objects[&legal_target].tapped,
                "{}: the artifact's ability resolved",
                case.id
            );
            assert!(
                artifact.state.objects[&artifact_source].tapped,
                "{}: the artifact paid its tap cost",
                case.id
            );
        }
    }
}
