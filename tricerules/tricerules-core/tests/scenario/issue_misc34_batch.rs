//! Actual-card semantics for five pinned Standard sequential spells.
//! Pinned Oracle and current Scryfall rulings reviewed 2026-09-22.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast_and_resolve(e: &mut GameEngine, card: &str) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc34_make_a_stand_semantics() {
    let mut e = engine(834_001);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(&mut e, "make_a_stand");
    assert_eq!(
        (e.effective_power(own), e.effective_toughness(own)),
        (Some(3), Some(2))
    );
    assert!(e.effective_has_keyword(own, Keyword::Indestructible));
    assert_eq!(
        (e.effective_power(opposing), e.effective_toughness(opposing)),
        (Some(2), Some(2))
    );
    assert!(!e.effective_has_keyword(opposing, Keyword::Indestructible));
    e.state.objects.get_mut(&own).unwrap().damage = 2;
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&own].zone, Zone::Battlefield);
    let later = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert_eq!(e.effective_power(later), Some(2));
    assert!(!e.effective_has_keyword(later, Keyword::Indestructible));
}

#[test]
fn issue_misc34_heroic_reinforcements_semantics() {
    let mut e = engine(834_010);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(&mut e, "heroic_reinforcements");
    let soldiers = battlefield_token_oids(&e, 0, "soldier_w_1_1");
    assert_eq!(soldiers.len(), 2);
    for soldier in soldiers {
        assert_eq!(
            (e.effective_power(soldier), e.effective_toughness(soldier)),
            (Some(2), Some(2))
        );
        assert!(e.effective_has_keyword(soldier, Keyword::Haste));
    }
    assert_eq!(
        (e.effective_power(own), e.effective_toughness(own)),
        (Some(3), Some(3))
    );
    assert!(e.effective_has_keyword(own, Keyword::Haste));
    assert_eq!(
        (e.effective_power(opposing), e.effective_toughness(opposing)),
        (Some(2), Some(2))
    );
    assert!(battlefield_token_oids(&e, 1, "soldier_w_1_1").is_empty());
    let later = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert_eq!(e.effective_power(later), Some(2));
    assert!(!e.effective_has_keyword(later, Keyword::Haste));
}

#[test]
fn issue_misc34_on_the_job_semantics() {
    let mut e = engine(834_020);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(&mut e, "on_the_job");
    assert_eq!(
        (e.effective_power(own), e.effective_toughness(own)),
        (Some(4), Some(3))
    );
    assert_eq!(
        (e.effective_power(opposing), e.effective_toughness(opposing)),
        (Some(2), Some(2))
    );
    let clues = battlefield_token_oids(&e, 0, "clue");
    assert_eq!(clues.len(), 1);
    assert_eq!(
        e.effective_power(clues[0]),
        None,
        "Clue is an artifact, not a creature"
    );
    assert!(battlefield_token_oids(&e, 1, "clue").is_empty());
}

#[test]
fn issue_misc34_the_crystals_chosen_semantics() {
    let mut e = engine(834_030);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(&mut e, "the_crystals_chosen");
    assert_eq!(
        e.state.objects[&own].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        e.state.objects[&opposing].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    let heroes = battlefield_token_oids(&e, 0, "hero_c_1_1");
    assert_eq!(heroes.len(), 4);
    for hero in heroes {
        assert_eq!(
            e.state.objects[&hero].counter_count(CounterKind::PlusOnePlusOne),
            1
        );
        assert_eq!(
            (e.effective_power(hero), e.effective_toughness(hero)),
            (Some(2), Some(2))
        );
    }
    assert!(battlefield_token_oids(&e, 1, "hero_c_1_1").is_empty());
}

#[test]
fn issue_misc34_deduce_semantics() {
    let mut e = engine(834_040);
    inject_card_into_hand(&mut e, 0, "deduce");
    let before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "deduce");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.turn_history.current.player(0).cards_drawn, 1);
    assert_eq!(
        e.state.players[0].hand.len(),
        before,
        "cast one spell and drew one card"
    );
    let clues = battlefield_token_oids(&e, 0, "clue");
    assert_eq!(clues.len(), 1);
    assert_eq!(e.effective_power(clues[0]), None);
    assert!(battlefield_token_oids(&e, 1, "clue").is_empty());
}
