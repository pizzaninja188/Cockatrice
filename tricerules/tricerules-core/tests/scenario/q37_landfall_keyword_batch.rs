//! Actual-card scenarios for the queue-v37 landfall keyword cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a triggers on a land entering under
//! the source controller; CR 611.2a/514.2 end the keyword grant at cleanup; the granted
//! keyword follows its own rule (702.7 first strike, 702.9 flying, 702.20 vigilance).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    keyword: Keyword,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "belligerent_whiptail",
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "fledgling_griffin",
            keyword: Keyword::Flying,
        },
        Case {
            id: "geyser_glider",
            keyword: Keyword::Flying,
        },
        Case {
            id: "snapping_creeper",
            keyword: Keyword::Vigilance,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn play_forest(e: &mut GameEngine, player: usize) {
    ensure_in_hand(e, player, "forest");
    let slot = hand_index_for_card(e, player, "forest");
    e.apply_scenario_command(player as i32, &play_land(slot))
        .expect("play Forest");
}

#[test]
fn q37_landfall_keyword_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(37_080_001);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    assert!(!e.effective_has_keyword(source, case.keyword));
    play_forest(&mut e, 0);
    assert!(
        !e.effective_has_keyword(source, case.keyword),
        "the trigger waits for resolution"
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(source, case.keyword));
    end_active_turn(&mut e, 0);
    assert!(!e.effective_has_keyword(source, case.keyword));
}

#[test]
fn q37_landfall_keyword_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(37_080_002);
    let source = inject_creature_with_stats(&mut e, 0, case.id, 4, 2);
    play_forest(&mut e, 0);
    assert!(!e.effective_has_keyword(source, case.keyword));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(source, case.keyword));
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(4), Some(2)),
        "the trigger does not change the body"
    );
}

#[test]
fn q37_landfall_keyword_cards_trigger_only_on_their_controllers_land() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_081_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        move_ready_to_battlefield(&mut e, 1, "forest");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            !e.effective_has_keyword(source, case.keyword),
            "{}: an opponent's land does not trigger landfall",
            case.id
        );
        move_ready_to_battlefield(&mut e, 0, "forest");
        resolve_entire_stack_two_player(&mut e);
        assert!(e.effective_has_keyword(source, case.keyword), "{}", case.id);
    }
}

#[test]
fn q37_landfall_keyword_cards_trigger_on_each_land_entry() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_082_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        move_ready_to_battlefield(&mut e, 0, "forest");
        resolve_entire_stack_two_player(&mut e);
        assert!(e.effective_has_keyword(source, case.keyword), "{}", case.id);
        move_ready_to_battlefield(&mut e, 0, "forest");
        assert!(
            !e.state.stack.is_empty(),
            "{}: the second land triggers landfall again",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert!(e.effective_has_keyword(source, case.keyword), "{}", case.id);
    }
}

#[test]
fn q37_landfall_keyword_cards_do_not_trigger_on_nonland_entries() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_083_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        inject_card_into_hand(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast Grizzly Bears");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            !e.effective_has_keyword(source, case.keyword),
            "{}: a creature entry does not trigger landfall",
            case.id
        );
    }
}
