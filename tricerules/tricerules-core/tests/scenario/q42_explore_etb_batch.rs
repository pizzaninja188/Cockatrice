//! Actual-card scenarios for the queue-v42 explore-on-entry creature cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on the stack;
//! CR 701.44a-c make the source explore during resolution, including the land-to-hand and
//! counter-plus-graveyard-or-top branches; CR 701.20a makes the reveal public.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    body: (u32, u32),
    keyword: Keyword,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "queens_agent",
            body: (3, 3),
            keyword: Keyword::Lifelink,
        },
        Case {
            id: "dire_fleet_interloper",
            body: (2, 2),
            keyword: Keyword::Menace,
        },
        Case {
            id: "siren_lookout",
            body: (1, 2),
            keyword: Keyword::Flying,
        },
        Case {
            id: "brazen_buccaneers",
            body: (2, 2),
            keyword: Keyword::Haste,
        },
        Case {
            id: "sunrise_seeker",
            body: (3, 3),
            keyword: Keyword::Vigilance,
        },
        Case {
            id: "emissary_of_sunrise",
            body: (2, 1),
            keyword: Keyword::FirstStrike,
        },
    ]
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
fn q42_explore_etb_canary_simplest_takes_a_revealed_land() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "siren_lookout")
        .expect("Siren Lookout case");
    let mut e = engine(42_050_001, case.id);
    let land = put_on_top(&mut e, 0, "forest");
    cast(&mut e, case.id);
    resolve_entire_stack_two_player(&mut e);
    let source = battlefield_object_for_card(&e, 0, case.id);
    assert_eq!(
        e.state.objects[&land].zone,
        Zone::Hand,
        "the revealed land goes to hand"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1)),
        "the land reveal adds no counter"
    );
    assert!(e.effective_has_keyword(source, Keyword::Flying));
}

#[test]
fn q42_explore_etb_canary_hardest_puts_a_counter_on_a_nonland() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "emissary_of_sunrise")
        .expect("Emissary of Sunrise case");
    let mut e = engine(42_050_002, case.id);
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
        (Some(case.body.0 + 1), Some(case.body.1 + 1)),
        "the nonland reveal adds +1/+1"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![nonland]))
        .expect("put the revealed nonland into the graveyard");
    assert_eq!(e.state.objects[&nonland].zone, Zone::Graveyard);
    assert!(e.effective_has_keyword(source, Keyword::FirstStrike));
}

#[test]
fn q42_explore_etb_cards_take_a_land_and_publish_their_keyword() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_051_000 + index as u64, case.id);
        let land = put_on_top(&mut e, 0, "forest");
        cast(&mut e, case.id);
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(e.state.objects[&land].zone, Zone::Hand, "{}", case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(source, case.keyword),
            "{}: printed keyword",
            case.id
        );
    }
}

#[test]
fn q42_explore_etb_cards_add_one_counter_and_offer_graveyard_or_top() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_052_000 + index as u64, case.id);
        let nonland = put_on_top(&mut e, 0, "storm_crow");
        cast(&mut e, case.id);
        let first = e.state.priority_player_id();
        e.apply_scenario_command(first, &pass())
            .expect("first pass into the explore trigger");
        let batch = e
            .apply_scenario_command(1 - first, &pass())
            .expect("resolve the explore trigger");
        assert_eq!(
            find_resolution_choice(&batch)
                .expect("nonland explore choice")
                .candidate_object_ids,
            [nonland],
            "{}",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![nonland]))
            .expect("put the revealed nonland into the graveyard");
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0 + 1), Some(case.body.1 + 1)),
            "{}: exactly one +1/+1 counter",
            case.id
        );

        let mut kept = engine(42_053_000 + index as u64, case.id);
        let nonland = put_on_top(&mut kept, 0, "storm_crow");
        cast(&mut kept, case.id);
        let first = kept.state.priority_player_id();
        kept.apply_scenario_command(first, &pass())
            .expect("first pass into the explore trigger");
        let batch = kept
            .apply_scenario_command(1 - first, &pass())
            .expect("resolve the explore trigger");
        assert_eq!(
            find_resolution_choice(&batch)
                .expect("nonland explore choice")
                .candidate_object_ids,
            [nonland]
        );
        kept.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("leave the revealed nonland on top");
        assert_eq!(
            kept.state.players[0].library.front().copied(),
            Some(nonland),
            "{}: the declined card remains on top",
            case.id
        );
        let source = battlefield_object_for_card(&kept, 0, case.id);
        assert_eq!(
            (
                kept.effective_power(source),
                kept.effective_toughness(source)
            ),
            (Some(case.body.0 + 1), Some(case.body.1 + 1)),
            "{}",
            case.id
        );
    }
}
