//! Actual-card scenarios for the reviewed `{T}: Untap target land.` family.
//!
//! Expectations are printed Oracle values. CR 107.5 and 118.3 require an
//! untapped source; CR 602.2b and 115.1c choose one land target on activation;
//! CR 701.26b untaps only a tapped permanent, and CR 608.2b leaves a stale
//! target unaffected.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "blossom_dryad",
        },
        Case {
            id: "juniper_order_druid",
        },
        Case { id: "ley_druid" },
        Case {
            id: "voyaging_satyr",
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

#[test]
fn tap_untap_land_batch_cards_untap_a_target_land() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_210_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        e.state.objects.get_mut(&land).expect("land").tapped = true;
        apply_ability(&mut e, 0, source, 0, target_object(land)).expect("activate at the land");
        assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
        assert!(
            e.state.objects[&land].tapped,
            "{}: the target untaps only on resolution",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert!(
            !e.state.objects[&land].tapped,
            "{}: the target land is untapped",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn tap_untap_land_batch_cards_accept_any_players_land() {
    for (index, case) in cases().into_iter().enumerate() {
        for (player, label) in [(0usize, "controller"), (1usize, "opponent")] {
            let mut e = engine(9_211_000 + index as u64 * 10 + player as u64, case.id);
            let source = move_ready_to_battlefield(&mut e, 0, case.id);
            let land = inject_permanent_on_battlefield(&mut e, player, "forest");
            e.state.objects.get_mut(&land).expect("land").tapped = true;
            apply_ability(&mut e, 0, source, 0, target_object(land))
                .unwrap_or_else(|error| panic!("{}: {label} land rejected: {error}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert!(
                !e.state.objects[&land].tapped,
                "{}: {label} land untapped",
                case.id
            );
        }
    }
}

#[test]
fn tap_untap_land_batch_cards_reject_nonland_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_212_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(artifact)).is_err(),
            "{}: an artifact is not a land target",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: rejected activation",
            case.id
        );
        assert!(!e.state.objects[&artifact].tapped, "{}", case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: a player is not a land target",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_untap_land_batch_cards_fizzle_a_stale_land_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_213_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        e.state.objects.get_mut(&land).expect("land").tapped = true;
        apply_ability(&mut e, 0, source, 0, target_object(land)).expect("activate at the land");
        e.state.players[1].battlefield.retain(|oid| *oid != land);
        e.state.objects.get_mut(&land).expect("land").zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(land);
        *e.state.zone_change_generation.entry(land).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Graveyard,
            "{}: the stale land stays in the graveyard",
            case.id
        );
        assert!(
            e.state.objects[&land].tapped,
            "{}: the stale land was not untapped",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_untap_land_batch_cards_require_an_untapped_source_and_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_214_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        e.state.objects.get_mut(&land).expect("land").tapped = true;
        apply_ability(&mut e, 0, source, 0, target_object(land)).expect("first activation");
        let second_land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        e.state.objects.get_mut(&second_land).expect("land").tapped = true;
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(second_land)).is_err(),
            "{}: a tapped source cannot activate again",
            case.id
        );

        let mut foreign = engine(9_215_000 + index as u64, case.id);
        let foreign_source = move_ready_to_battlefield(&mut foreign, 0, case.id);
        let foreign_land = inject_permanent_on_battlefield(&mut foreign, 0, "forest");
        foreign
            .state
            .objects
            .get_mut(&foreign_land)
            .expect("land")
            .tapped = true;
        foreign
            .apply_scenario_command(0, &pass())
            .expect("the controller passes priority");
        assert!(
            apply_ability(
                &mut foreign,
                1,
                foreign_source,
                0,
                target_object(foreign_land)
            )
            .is_err(),
            "{}: another player cannot activate it",
            case.id
        );
    }
}
