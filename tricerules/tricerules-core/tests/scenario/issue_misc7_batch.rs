//! Reviewed direct-RON triggered-ability scenarios: Malamet Brawler, Nori Teller of Tales,
//! Moonglove Extractor, Good-Fortune Unicorn, Lightless Evangel, Spirit Mascot, Tanufel
//! Rimespeaker and Guttersnipe.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! CR 119 (life loss), CR 120.3 (damage), CR 122.1 (counters), CR 400.7 (zone changes),
//! CR 508 (attack triggers), CR 603.2c (one trigger per simultaneous event), and CR 611.2c.

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::Keyword;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, TargetRef, TargetRefKind};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn plus_one(e: &GameEngine, oid: u32) -> u32 {
    e.state.objects[&oid].counter_count(CounterKind::PlusOnePlusOne)
}

#[test]
fn issue_misc7_malamet_brawler_grants_trample_to_an_attacker() {
    let mut e = GameEngine::new(731_001, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let brawler = inject_creature_on_battlefield(&mut e, 0, "malamet_brawler");
    let fellow = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bench = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    e.apply_command(0, &declare_attackers(vec![brawler, fellow]))
        .expect("declare two attackers");
    assert_eq!(e.state.pending_triggers.len(), 1);
    assert!(
        e.apply_command(0, &choose_trigger_target(bench)).is_err(),
        "a nonattacking creature is not a legal target"
    );
    semantic::accepted(&mut e, 0, &choose_trigger_target(fellow));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(fellow, Keyword::Trample));
    assert!(!e.effective_has_keyword(brawler, Keyword::Trample));
}

#[test]
fn issue_misc7_nori_grants_first_strike_to_an_attacker() {
    let mut e = GameEngine::new(731_002, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let nori = inject_creature_on_battlefield(&mut e, 0, "nori,_teller_of_tales");
    let fellow = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![nori, fellow]))
        .expect("declare two attackers");
    semantic::accepted(&mut e, 0, &choose_trigger_target(fellow));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(fellow, Keyword::FirstStrike));
    assert!(!e.effective_has_keyword(nori, Keyword::FirstStrike));
}

#[test]
fn issue_misc7_moonglove_extractor_draws_and_loses_life_on_attack() {
    let mut e = GameEngine::new(731_003, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let extractor = inject_creature_on_battlefield(&mut e, 0, "moonglove_extractor");
    let life_before = e.state.players[0].life;
    let hand_before = e.state.players[0].hand.len();
    e.apply_command(0, &declare_attackers(vec![extractor]))
        .expect("declare the Extractor as an attacker");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(e.state.players[0].life, life_before - 1);
}

#[test]
fn issue_misc7_good_fortune_unicorn_counters_an_entering_creature() {
    let mut e = engine(731_004);
    let unicorn = inject_creature_on_battlefield(&mut e, 0, "good-fortune_unicorn");
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "grizzly_bears");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    let entrant = *e.state.players[0]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == "grizzly_bears")
        .expect("the Bear entered");
    assert_eq!(
        plus_one(&e, entrant),
        1,
        "the entering creature gets the counter"
    );
    assert_eq!(plus_one(&e, unicorn), 0, "the Unicorn itself does not");

    // A noncreature permanent entering does not trigger the creature watcher.
    let mut no_trigger = engine(731_014);
    inject_creature_on_battlefield(&mut no_trigger, 0, "good-fortune_unicorn");
    inject_card_into_hand(&mut no_trigger, 0, "swiftfoot_boots");
    grant_pool(&mut no_trigger, 0);
    let slot = hand_index_for_card(&no_trigger, 0, "swiftfoot_boots");
    semantic::accepted(&mut no_trigger, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut no_trigger);
    assert_eq!(no_trigger.state.pending_triggers.len(), 0);
    assert_eq!(no_trigger.state.stack.len(), 0);
}

#[test]
fn issue_misc7_lightless_evangel_counters_on_a_sacrifice() {
    // Sacrificing another creature triggers the Evangel.
    let mut e = engine(731_005);
    let evangel = inject_creature_on_battlefield(&mut e, 0, "lightless_evangel");
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let acolyte = inject_creature_on_battlefield(&mut e, 0, "acolyte_of_aclazotz");
    grant_pool(&mut e, 0);
    semantic::accepted(
        &mut e,
        0,
        &activate_ability_with_costs(
            acolyte,
            0,
            vec![],
            vec![permanent_cost_selection(1, fodder)],
        ),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, evangel), 1);

    // Sacrificing a land (not a creature or artifact) does not trigger it.
    let mut no_trigger = engine(731_015);
    let evangel = inject_creature_on_battlefield(&mut no_trigger, 0, "lightless_evangel");
    let forest = inject_permanent_on_battlefield(&mut no_trigger, 0, "forest");
    let scrapper = inject_creature_on_battlefield(&mut no_trigger, 0, "slagdrill_scrapper");
    grant_pool(&mut no_trigger, 0);
    semantic::accepted(
        &mut no_trigger,
        0,
        &activate_ability_with_costs(
            scrapper,
            0,
            vec![],
            vec![permanent_cost_selection(2, forest)],
        ),
    );
    resolve_entire_stack_two_player(&mut no_trigger);
    assert_eq!(plus_one(&no_trigger, evangel), 0);
}

#[test]
fn issue_misc7_spirit_mascot_counters_when_a_card_leaves_the_graveyard() {
    let mut e = engine(731_006);
    let mascot = inject_creature_on_battlefield(&mut e, 0, "spirit_mascot");
    let vacuum = inject_permanent_on_battlefield(&mut e, 0, "ghost_vacuum");
    let own_card = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    semantic::accepted(
        &mut e,
        0,
        &activate_ability(vacuum, 0, target_object(own_card)),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        plus_one(&e, mascot),
        1,
        "one card left the controller's graveyard, so the ability triggers once"
    );

    // A card leaving an opponent's graveyard is outside the Controller owner scope.
    let mut no_trigger = engine(731_016);
    let mascot = inject_creature_on_battlefield(&mut no_trigger, 0, "spirit_mascot");
    let vacuum = inject_permanent_on_battlefield(&mut no_trigger, 0, "ghost_vacuum");
    let their_card = inject_graveyard_card(&mut no_trigger, 1, "grizzly_bears");
    semantic::accepted(
        &mut no_trigger,
        0,
        &activate_ability(vacuum, 0, target_object(their_card)),
    );
    resolve_entire_stack_two_player(&mut no_trigger);
    assert_eq!(plus_one(&no_trigger, mascot), 0);
}

#[test]
fn issue_misc7_tanufel_rimespeaker_draws_for_a_four_plus_spell() {
    let mut e = engine(731_007);
    inject_creature_on_battlefield(&mut e, 0, "tanufel_rimespeaker");
    inject_card_into_hand(&mut e, 0, "fateful_discovery");
    grant_pool(&mut e, 0);
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "fateful_discovery");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the mana-value-5 spell leaves hand and the trigger draws one"
    );

    // A spell with mana value below 4 does not trigger the watcher.
    let mut no_trigger = engine(731_017);
    inject_creature_on_battlefield(&mut no_trigger, 0, "tanufel_rimespeaker");
    inject_card_into_hand(&mut no_trigger, 0, "vampiric_rites");
    grant_pool(&mut no_trigger, 0);
    let hand_before = no_trigger.state.players[0].hand.len();
    let slot = hand_index_for_card(&no_trigger, 0, "vampiric_rites");
    semantic::accepted(&mut no_trigger, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut no_trigger);
    assert_eq!(no_trigger.state.players[0].hand.len(), hand_before - 1);
}

#[test]
fn issue_misc7_guttersnipe_damages_each_opponent_for_an_instant_or_sorcery() {
    let mut e = engine(731_008);
    inject_creature_on_battlefield(&mut e, 0, "guttersnipe");
    inject_card_into_hand(&mut e, 0, "divination");
    grant_pool(&mut e, 0);
    let p1_before = e.state.players[1].life;
    let slot = hand_index_for_card(&e, 0, "divination");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        p1_before - 2,
        "2 damage to the opponent"
    );

    // A creature spell does not trigger the instant-or-sorcery watcher.
    let mut no_trigger = engine(731_018);
    inject_creature_on_battlefield(&mut no_trigger, 0, "guttersnipe");
    inject_card_into_hand(&mut no_trigger, 0, "grizzly_bears");
    grant_pool(&mut no_trigger, 0);
    let p1_before = no_trigger.state.players[1].life;
    let slot = hand_index_for_card(&no_trigger, 0, "grizzly_bears");
    semantic::accepted(&mut no_trigger, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut no_trigger);
    assert_eq!(no_trigger.state.players[1].life, p1_before);
}
