//! Actual-card scenarios for the reviewed `{2}, {T}: Tap target creature` family.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 govern the activated
//! ability and its cost, CR 118.3 the mana payment, CR 701.26 tapping, and
//! CR 608.2b resolution revalidation.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    body: Option<(u32, u32)>,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "fan_bearer",
            cast: ManaGift {
                w: 1,
                ..Default::default()
            },
            body: Some((1, 2)),
        },
        Case {
            id: "gavony_trapper",
            cast: ManaGift {
                w: 1,
                ..Default::default()
            },
            body: Some((0, 2)),
        },
        Case {
            id: "trip_noose",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            body: None,
        },
    ]
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

fn place_source(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    move_ready_to_battlefield(e, 0, case.id)
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

#[test]
fn tap_target_two_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_030_000 + index as u64);
        let source = place_source(&mut e, &case);
        if let Some((power, toughness)) = case.body {
            assert_eq!(e.effective_power(source), Some(power), "{}", case.id);
            assert_eq!(
                e.effective_toughness(source),
                Some(toughness),
                "{}",
                case.id
            );
        }
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
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
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: the {{2}} cost was paid",
            case.id
        );
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
fn tap_target_two_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_031_000 + index as u64);
        let source = place_source(&mut e, &case);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let before = pool(&e);
        for (illegal, label) in [
            (target_object(artifact), "artifact"),
            (target_player(1), "player"),
        ] {
            let before_command = e.state.command_index;
            assert!(
                e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, illegal))
                    .is_err(),
                "{}: a {label} is not a creature target",
                case.id
            );
            assert_eq!(pool(&e), before, "{}: rejected {label} activation", case.id);
            assert_eq!(e.state.command_index, before_command, "{}", case.id);
        }
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .expect("the same source still activates for a legal creature");
        resolve_entire_stack_two_player(&mut e);
        assert!(e.state.objects[&target].tapped, "{}", case.id);
    }
}

#[test]
fn tap_target_two_batch_cards_require_the_printed_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_032_000 + index as u64);
        let source = place_source(&mut e, &case);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let before = e.state.command_index;
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(target))
            )
            .is_err(),
            "{}: two generic mana are required",
            case.id
        );
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .expect("activation with the printed cost");
        resolve_entire_stack_two_player(&mut e);
        let after = e.state.command_index;
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(target))
            )
            .is_err(),
            "{}: the tapped source cannot pay Tap again",
            case.id
        );
        assert_eq!(e.state.command_index, after, "{}", case.id);
    }
}

#[test]
fn tap_target_two_batch_cards_fizzle_when_the_target_left() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_033_000 + index as u64);
        let source = place_source(&mut e, &case);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .expect("activate the tap ability");
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the cost remains paid",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
