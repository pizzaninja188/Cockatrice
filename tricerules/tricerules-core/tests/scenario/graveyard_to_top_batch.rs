//! Actual-card scenarios for the queue-v27 graveyard-to-top cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 and 601.2c choose one card in
//! the caster's graveyard; CR 400.7 gives a card that changes zones a new object
//! identity; CR 401.4 puts that exact card on top of its owner's library; CR 608.2b
//! fizzles the spell when the only target is illegal.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRef;

struct Case {
    id: &'static str,
    target: &'static str,
    other: &'static str,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "false_mourning",
            target: "grizzly_bears",
            other: "lightning_bolt",
        },
        Case {
            id: "reclaim",
            target: "forest",
            other: "divination",
        },
        Case {
            id: "salvage",
            target: "bonesplitter",
            other: "grizzly_bears",
        },
    ]
}

fn graveyard_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Graveyard as i32,
        ..Default::default()
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn prepare(e: &mut GameEngine, case: &Case) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(
        e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
}

fn cast_to_top(e: &mut GameEngine, case: &Case, target: u32) -> Result<(), String> {
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(target)]))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn graveyard_to_top_batch_cards_put_the_chosen_card_on_top() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_080_000 + index as u64, case.id);
        let target = inject_graveyard_card(&mut e, 0, case.target);
        let other = inject_graveyard_card(&mut e, 0, case.other);
        prepare(&mut e, &case);
        let library_before = e.state.players[0].library.len();
        assert!(cast_to_top(&mut e, &case, target).is_ok(), "{}", case.id);
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Library, "{}", case.id);
        assert_eq!(
            e.state.players[0].library[0], target,
            "{}: the chosen card is on top",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before + 1,
            "{}: the graveyard card moved to the library",
            case.id
        );
        assert_eq!(e.state.objects[&other].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(count_card_id_in_graveyard(&e, 0, case.id), 1, "{}", case.id);
    }
}

#[test]
fn graveyard_to_top_batch_cards_reject_an_opponents_graveyard_and_a_missing_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_081_000 + index as u64, case.id);
        let opponent = inject_graveyard_card(&mut e, 1, case.target);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        let hand_before = e.state.players[0].hand.len();
        let pool_before = e.state.players[0].mana_pool.green;
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(opponent)]))
                .is_err(),
            "{}: only the caster's graveyard is legal",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the graveyard target is mandatory",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.players[0].mana_pool.green, pool_before,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&opponent].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&opponent),
            "{}",
            case.id
        );
    }
}

#[test]
fn graveyard_to_top_batch_cards_fizzle_when_the_target_leaves_the_graveyard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_082_000 + index as u64, case.id);
        let target = inject_graveyard_card(&mut e, 0, case.target);
        let other = inject_graveyard_card(&mut e, 0, case.other);
        prepare(&mut e, &case);
        let library_before = e.state.players[0].library.clone();
        assert!(cast_to_top(&mut e, &case, target).is_ok(), "{}", case.id);
        e.state.players[0].graveyard.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{}", case.id);
        assert_eq!(
            e.state.players[0].library, library_before,
            "{}: the fizzled spell changes no library card",
            case.id
        );
        assert_eq!(e.state.objects[&other].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(count_card_id_in_graveyard(&e, 0, case.id), 1, "{}", case.id);
    }
}
