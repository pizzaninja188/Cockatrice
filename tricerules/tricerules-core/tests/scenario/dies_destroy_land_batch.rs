//! Actual-card scenarios for the queue-v27 dies-destroy-land cohort.
//!
//! Expectations are printed Oracle values. CR 700.4 makes dies mean battlefield to
//! graveyard; CR 603.6c triggers from that event; CR 115.1 makes the chosen land a
//! target; CR 701.8 destroys it. Goblin Gardener's pinned ruling confirms the
//! ability is mandatory and may target your own land when the opponent has none.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    body: (u32, u32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "fire_snake",
            body: (3, 1),
        },
        Case {
            id: "goblin_gardener",
            body: (2, 1),
        },
        Case {
            id: "polluted_dead",
            body: (3, 3),
        },
    ]
}

fn choose_trigger_target(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[card, "murder", "swords_to_plowshares"]),
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

fn kill_source(e: &mut GameEngine, source: u32, card: &str) {
    ensure_in_hand(e, 0, "murder");
    give_mana(
        e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(murder, target_object(source)))
        .unwrap_or_else(|error| panic!("{card}: destroy the source: {error:?}"));
    resolve_entire_stack_two_player(e);
}

#[test]
fn dies_destroy_land_batch_cards_destroy_the_chosen_land_of_either_player() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_070_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        let opponent_land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        let other_land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        kill_source(&mut e, source, case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.pending_triggers.len(),
            1,
            "{}: one dies trigger",
            case.id
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_object(opponent_land)))
            .expect("choose the opponent's land");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&opponent_land].zone,
            Zone::Graveyard,
            "{}: the chosen land is destroyed",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&opponent_land),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&other_land].zone,
            Zone::Battlefield,
            "{}: only the chosen land is destroyed",
            case.id
        );
    }
}

#[test]
fn dies_destroy_land_batch_cards_must_choose_your_own_land_when_the_opponent_has_none() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_071_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let own_land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        assert!(
            e.state.players[1]
                .battlefield
                .iter()
                .all(|oid| !e.state.objects[oid].card_id.contains("forest")),
            "{}",
            case.id
        );
        kill_source(&mut e, source, case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(own_land)))
            .expect("choose your own land");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&own_land].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&own_land),
            "{}",
            case.id
        );
    }
}

#[test]
fn dies_destroy_land_batch_cards_do_not_trigger_when_exiled() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_072_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        ensure_in_hand(&mut e, 0, "swords_to_plowshares");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let swords = hand_index_for_card(&e, 0, "swords_to_plowshares");
        e.apply_scenario_command(0, &cast_spell(swords, target_object(source)))
            .expect("exile the source");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&source].zone, Zone::Exile, "{}", case.id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: leaving for exile is not dying",
            case.id
        );
    }
}

#[test]
fn dies_destroy_land_batch_cards_fizzle_when_the_chosen_land_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_073_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        kill_source(&mut e, source, case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(land)))
            .expect("choose the land");
        e.state.players[1].battlefield.retain(|oid| *oid != land);
        e.state.objects.get_mut(&land).expect("land").zone = Zone::Exile;
        *e.state.zone_change_generation.entry(land).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&land].zone, Zone::Exile, "{}", case.id);
        assert!(e.state.pending_triggers.is_empty(), "{}", case.id);
    }
}
