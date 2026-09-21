//! Reviewed token-creation scenarios: Elder Auntie, Dragon Trainer, Glimmerburst, Release the Dogs
//! and Hop to It.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 111.1 (tokens),
//! 121.1 (draw), 603.6a (entry triggers), and 702.9 (flying).

use super::helpers::*;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast_and_resolve(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc3_elder_auntie() {
    let mut e = engine(727_001);
    cast_and_resolve(&mut e, "elder_auntie", vec![]);
    let goblins = battlefield_token_oids(&e, 0, "goblin_br_1_1");
    assert_eq!(
        goblins.len(),
        1,
        "the entry trigger creates one Goblin token"
    );
    assert_eq!(e.effective_power(goblins[0]), Some(1));
    assert_eq!(e.effective_toughness(goblins[0]), Some(1));
}

#[test]
fn issue_misc3_dragon_trainer() {
    let mut e = engine(727_002);
    cast_and_resolve(&mut e, "dragon_trainer", vec![]);
    let dragons = battlefield_token_oids(&e, 0, "dragon_r_4_4_flying");
    assert_eq!(
        dragons.len(),
        1,
        "the entry trigger creates one Dragon token"
    );
    assert_eq!(e.effective_power(dragons[0]), Some(4));
    assert_eq!(e.effective_toughness(dragons[0]), Some(4));
    assert!(
        e.effective_has_keyword(dragons[0], tricerules_cards::Keyword::Flying),
        "the Dragon token has flying"
    );
}

#[test]
fn issue_misc3_glimmerburst() {
    let mut e = engine(727_003);
    inject_card_into_hand(&mut e, 0, "glimmerburst");
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "glimmerburst");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 2,
        "the spell draws two cards"
    );
    assert_eq!(
        battlefield_token_oids(&e, 0, "glimmer_w_1_1").len(),
        1,
        "the spell creates one Glimmer token"
    );
}

#[test]
fn issue_misc3_release_the_dogs() {
    let mut e = engine(727_004);
    cast_and_resolve(&mut e, "release_the_dogs", vec![]);
    assert_eq!(
        battlefield_token_oids(&e, 0, "dog_w_1_1").len(),
        4,
        "the spell creates exactly four Dog tokens"
    );
}

#[test]
fn issue_misc3_hop_to_it() {
    let mut e = engine(727_005);
    cast_and_resolve(&mut e, "hop_to_it", vec![]);
    assert_eq!(
        battlefield_token_oids(&e, 0, "rabbit_w_1_1").len(),
        3,
        "the spell creates exactly three Rabbit tokens"
    );
}
