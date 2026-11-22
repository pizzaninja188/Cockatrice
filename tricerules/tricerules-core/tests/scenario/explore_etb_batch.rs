//! Actual-card scenarios for the reviewed ETB Explore cohort.
//!
//! Expectations are printed Oracle text. CR 603.6a puts the entry trigger on the
//! stack; CR 701.44a-c make the source explore during resolution, including with
//! last-known information after it leaves; CR 701.20a makes the reveal public; a
//! revealed land goes to hand, and a revealed nonland adds a +1/+1 counter before
//! CR 608.2d has the controller choose graveyard or top.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "ixallis_diviner",
            power: 0,
            toughness: 3,
        },
        Case {
            id: "merfolk_branchwalker",
            power: 2,
            toughness: 1,
        },
        Case {
            id: "seekers_squire",
            power: 1,
            toughness: 2,
        },
        Case {
            id: "tishanas_wayfinder",
            power: 2,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "murder"]),
        deck_with("island", &[]),
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

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn cast(e: &mut GameEngine, card: &str) {
    ensure_in_hand(e, 0, card);
    give_mana(
        e,
        0,
        ManaGift {
            w: 1,
            u: 1,
            b: 1,
            r: 1,
            g: 1,
            c: 6,
        },
    );
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the exploring creature");
    pass_both_players(e);
}

#[test]
fn explore_etb_batch_cards_explore_on_entry_and_take_the_land() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_600_000 + index as u64, case.id);
        let land = put_on_top(&mut e, 0, "forest");
        cast(&mut e, case.id);
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the land reveal adds no counter",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Hand,
            "{}: the revealed land goes to hand",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}",
            case.id
        );
    }
}

#[test]
fn explore_etb_batch_cards_put_a_counter_on_a_nonland_and_choose_graveyard_or_top() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_610_000 + index as u64, case.id);
        let nonland = put_on_top(&mut e, 0, "storm_crow");
        cast(&mut e, case.id);
        let first = e.state.priority_player_id();
        e.apply_scenario_command(first, &pass())
            .expect("first pass into the explore trigger");
        let batch = e
            .apply_scenario_command(1 - first, &pass())
            .expect("resolve the explore trigger");
        let choice = find_resolution_choice(&batch).expect("nonland explore choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.candidate_object_ids, [nonland]);
        assert!(choice.public_reveal.is_some());
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power + 1), Some(case.toughness + 1)),
            "{}: the nonland reveal adds +1/+1",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![nonland]))
            .expect("put the revealed nonland into the graveyard");
        assert_eq!(
            e.state.objects[&nonland].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );

        let mut top = engine(10_620_000 + index as u64, case.id);
        let kept = put_on_top(&mut top, 0, "storm_crow");
        cast(&mut top, case.id);
        let first = top.state.priority_player_id();
        top.apply_scenario_command(first, &pass())
            .expect("first pass into the explore trigger");
        let batch = top
            .apply_scenario_command(1 - first, &pass())
            .expect("resolve the explore trigger");
        assert_eq!(
            find_resolution_choice(&batch)
                .expect("nonland explore choice")
                .candidate_object_ids,
            [kept]
        );
        top.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("leave the revealed nonland on top");
        assert_eq!(top.state.objects[&kept].zone, Zone::Library, "{}", case.id);
        assert_eq!(
            top.state.players[0].library.front().copied(),
            Some(kept),
            "{}: the declined card remains on top",
            case.id
        );
        let source = battlefield_object_for_card(&top, 0, case.id);
        assert_eq!(
            (top.effective_power(source), top.effective_toughness(source)),
            (Some(case.power + 1), Some(case.toughness + 1)),
            "{}",
            case.id
        );
    }
}

#[test]
fn explore_etb_batch_cards_do_not_explore_when_they_do_not_enter() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_630_000 + index as u64, case.id);
        let _source = inject_creature_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: no entry event was generated",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn explore_etb_batch_cards_explore_with_last_known_information_after_they_leave() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_640_000 + index as u64, case.id);
        let nonland = put_on_top(&mut e, 0, "storm_crow");
        cast(&mut e, case.id);
        let source = battlefield_object_for_card(&e, 0, case.id);
        ensure_in_hand(&mut e, 0, "murder");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        );
        let murder = hand_index_for_card(&e, 0, "murder");
        e.apply_scenario_command(0, &cast_spell(murder, target_object(source)))
            .expect("destroy the exploring source before its trigger resolves");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: the source left before exploring",
            case.id
        );
        let pending = e
            .state
            .pending_resolution
            .as_ref()
            .expect("the explore continuation still offers its nonland choice");
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryLook);
        assert_eq!(pending.presentation.candidates, [nonland]);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![nonland]))
            .expect("put the revealed nonland into the graveyard");
        assert_eq!(
            e.state.objects[&nonland].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}: a departed source receives no counter",
            case.id
        );
    }
}
