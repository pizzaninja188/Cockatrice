//! Reviewed activated-ability and enters-watcher scenarios: Vampire Neonate, Taxi Driver, Daring
//! Mechanic, Tanglespan Lookout, Fateful Discovery and Slagdrill Scrapper.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! 119/120 (life), 121.1 (draw), 122.1 (counters), 602 (activated abilities), 603.6 (entry
//! triggers), and 611.2c (until end of turn).

use super::helpers::*;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

#[test]
fn issue_misc4_vampire_neonate() {
    let mut e = engine(728_001);
    let neonate = inject_creature_on_battlefield(&mut e, 0, "vampire_neonate");
    let p0_before = e.state.players[0].life;
    let p1_before = e.state.players[1].life;
    semantic::accepted(&mut e, 0, &activate_ability(neonate, 0, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        p1_before - 1,
        "each opponent loses one"
    );
    assert_eq!(
        e.state.players[0].life,
        p0_before + 1,
        "the controller gains one"
    );
    assert!(e.state.objects[&neonate].tapped, "the tap cost was paid");
}

#[test]
fn issue_misc4_taxi_driver() {
    let mut e = engine(728_002);
    let driver = inject_creature_on_battlefield(&mut e, 0, "taxi_driver");
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    semantic::accepted(
        &mut e,
        0,
        &activate_ability(driver, 0, target_object(target)),
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(target, tricerules_cards::Keyword::Haste));

    // A noncreature permanent is not a legal target.
    let mut bad = engine(728_012);
    let driver = inject_creature_on_battlefield(&mut bad, 0, "taxi_driver");
    let boots = inject_permanent_on_battlefield(&mut bad, 0, "swiftfoot_boots");
    assert!(
        bad.apply_command(0, &activate_ability(driver, 0, target_object(boots)))
            .is_err(),
        "an artifact is not a creature"
    );
}

#[test]
fn issue_misc4_daring_mechanic() {
    let mut e = engine(728_003);
    let mechanic = inject_creature_on_battlefield(&mut e, 0, "daring_mechanic");
    let wagon = inject_permanent_on_battlefield(&mut e, 0, "lumbering_worldwagon");
    grant_pool(&mut e, 0);
    semantic::accepted(
        &mut e,
        0,
        &activate_ability(mechanic, 0, target_object(wagon)),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&wagon]
            .counter_count(tricerules_cards::primitives::CounterKind::PlusOnePlusOne),
        1,
        "a Vehicle is a legal target"
    );

    // A plain creature with neither subtype is not a legal target.
    let mut bad = engine(728_013);
    let mechanic = inject_creature_on_battlefield(&mut bad, 0, "daring_mechanic");
    let bear = inject_creature_on_battlefield(&mut bad, 0, "grizzly_bears");
    assert!(
        bad.apply_command(0, &activate_ability(mechanic, 0, target_object(bear)))
            .is_err(),
        "a creature that is neither a Mount nor a Vehicle is not a legal target"
    );
}

#[test]
fn issue_misc4_tanglespan_lookout() {
    // An Aura entering draws a card.
    let mut e = engine(728_004);
    inject_creature_on_battlefield(&mut e, 0, "tanglespan_lookout");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "pacifism");
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "pacifism");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(bear)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the Aura leaves hand and the trigger draws one"
    );

    // A creature entering does not trigger the Aura watcher.
    let mut no_draw = engine(728_014);
    inject_creature_on_battlefield(&mut no_draw, 0, "tanglespan_lookout");
    inject_card_into_hand(&mut no_draw, 0, "grizzly_bears");
    let hand_before = no_draw.state.players[0].hand.len();
    let slot = hand_index_for_card(&no_draw, 0, "grizzly_bears");
    semantic::accepted(&mut no_draw, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut no_draw);
    assert_eq!(no_draw.state.players[0].hand.len(), hand_before - 1);
}

#[test]
fn issue_misc4_fateful_discovery() {
    // An artifact entering draws a card.
    let mut e = engine(728_005);
    inject_permanent_on_battlefield(&mut e, 0, "fateful_discovery");
    inject_card_into_hand(&mut e, 0, "swiftfoot_boots");
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "swiftfoot_boots");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the artifact leaves hand and the trigger draws one"
    );

    // A creature entering does not trigger the artifact watcher.
    let mut no_draw = engine(728_015);
    inject_permanent_on_battlefield(&mut no_draw, 0, "fateful_discovery");
    inject_card_into_hand(&mut no_draw, 0, "grizzly_bears");
    let hand_before = no_draw.state.players[0].hand.len();
    let slot = hand_index_for_card(&no_draw, 0, "grizzly_bears");
    semantic::accepted(&mut no_draw, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut no_draw);
    assert_eq!(no_draw.state.players[0].hand.len(), hand_before - 1);
}

#[test]
fn issue_misc4_slagdrill_scrapper() {
    // Sacrificing a land draws a card.
    let mut e = engine(728_006);
    let scrapper = inject_creature_on_battlefield(&mut e, 0, "slagdrill_scrapper");
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    grant_pool(&mut e, 0);
    let hand_before = e.state.players[0].hand.len();
    semantic::accepted(
        &mut e,
        0,
        &activate_ability_with_costs(
            scrapper,
            0,
            vec![],
            vec![permanent_cost_selection(2, forest)],
        ),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&forest].zone,
        tricerules_core::Zone::Graveyard,
        "the land was sacrificed"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "draw a card"
    );

    // With no other artifact or land, the sacrifice cost cannot be paid.
    let mut alone = engine(728_016);
    let scrapper = inject_creature_on_battlefield(&mut alone, 0, "slagdrill_scrapper");
    grant_pool(&mut alone, 0);
    assert!(
        alone
            .apply_command(0, &activate_ability(scrapper, 0, vec![]))
            .is_err(),
        "the source is the only artifact and is excluded from the sacrifice"
    );
}
