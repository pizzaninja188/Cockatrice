//! Reviewed spells scenarios: Playful Shove, Risky Shortcut, Blight Rot, Bounce Off, Plunge into
//! Winter and Mind Spring.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 107.3 (X), 115
//! (targets), 119/120 (life and damage), 121.1 (draw), 122.1 (counters), 701.18 (scry), 701.26
//! (tap), and 400.7 (return to owner's hand).

use super::helpers::*;
use tricerules_core::Zone;

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
fn issue_misc2_playful_shove() {
    let mut e = engine(726_001);
    let p1_before = e.state.players[1].life;
    inject_card_into_hand(&mut e, 0, "playful_shove");
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "playful_shove");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_player_damage(1, 1)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        p1_before - 1,
        "one damage to the player"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 1,
        "the spell leaves hand and draws one"
    );

    // If the only target is illegal at resolution, the spell fizzles: no damage, no draw.
    let mut fizzle = engine(726_011);
    let target = inject_creature_on_battlefield(&mut fizzle, 1, "grizzly_bears");
    inject_card_into_hand(&mut fizzle, 0, "playful_shove");
    let hand_before = fizzle.state.players[0].hand.len();
    let p1_before = fizzle.state.players[1].life;
    let slot = hand_index_for_card(&fizzle, 0, "playful_shove");
    semantic::accepted(&mut fizzle, 0, &cast_spell(slot, target_object(target)));
    fizzle.state.players[1]
        .battlefield
        .retain(|id| *id != target);
    fizzle.state.players[1].graveyard.push(target);
    fizzle.state.objects.get_mut(&target).expect("object").zone = Zone::Graveyard;
    *fizzle
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(
        fizzle.state.players[1].life, p1_before,
        "no damage on a fizzle"
    );
    assert_eq!(
        fizzle.state.players[0].hand.len(),
        hand_before - 1,
        "no draw on a fizzle"
    );
}

#[test]
fn issue_misc2_risky_shortcut() {
    let mut e = engine(726_002);
    inject_card_into_hand(&mut e, 0, "risky_shortcut");
    let hand_before = e.state.players[0].hand.len();
    let p0_life = e.state.players[0].life;
    let p1_life = e.state.players[1].life;
    let slot = hand_index_for_card(&e, 0, "risky_shortcut");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 2,
        "draw two"
    );
    assert_eq!(e.state.players[0].life, p0_life - 2, "the caster loses two");
    assert_eq!(
        e.state.players[1].life,
        p1_life - 2,
        "the opponent loses two"
    );
}

#[test]
fn issue_misc2_blight_rot() {
    let mut e = engine(726_003);
    let angel = inject_creature_with_stats(&mut e, 1, "serra_angel", 4, 4);
    cast_and_resolve(&mut e, "blight_rot", target_object(angel));
    assert_eq!(
        e.state.objects[&angel].zone,
        Zone::Graveyard,
        "four -1/-1 counters reduce a 4/4 to 0/0"
    );

    // A noncreature permanent is not a legal target.
    let mut bad = engine(726_013);
    let boots = inject_permanent_on_battlefield(&mut bad, 1, "swiftfoot_boots");
    inject_card_into_hand(&mut bad, 0, "blight_rot");
    let slot = hand_index_for_card(&bad, 0, "blight_rot");
    assert!(
        bad.apply_command(0, &cast_spell(slot, target_object(boots)))
            .is_err(),
        "an artifact is not a creature"
    );
}

#[test]
fn issue_misc2_bounce_off() {
    let mut e = engine(726_004);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(&mut e, "bounce_off", target_object(bear));
    assert_eq!(
        e.state.objects[&bear].zone,
        Zone::Hand,
        "the creature returns to its owner's hand"
    );

    // A Vehicle is a legal target too.
    let mut vehicle = engine(726_024);
    let wagon = inject_permanent_on_battlefield(&mut vehicle, 1, "lumbering_worldwagon");
    cast_and_resolve(&mut vehicle, "bounce_off", target_object(wagon));
    assert_eq!(
        vehicle.state.objects[&wagon].zone,
        Zone::Hand,
        "the Vehicle returns to its owner's hand"
    );

    // A land is not a legal target.
    let mut land = engine(726_014);
    let forest = inject_permanent_on_battlefield(&mut land, 1, "forest");
    inject_card_into_hand(&mut land, 0, "bounce_off");
    let slot = hand_index_for_card(&land, 0, "bounce_off");
    assert!(
        land.apply_command(0, &cast_spell(slot, target_object(forest)))
            .is_err(),
        "a land is neither a creature nor a Vehicle"
    );
}

#[test]
fn issue_misc2_plunge_into_winter() {
    let mut e = engine(726_005);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "plunge_into_winter");
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "plunge_into_winter");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(bear)));
    e.apply_command(0, &pass()).expect("p0 pass");
    let batch = e.apply_command(1, &pass()).expect("the scry parks");
    let choice = find_resolution_choice(&batch).expect("scry choice");
    assert_eq!(
        choice.choice_kind(),
        tricerules_proto::ruled::v1::ChoiceKind::LibraryTop
    );
    assert!(
        e.state.objects[&bear].tapped,
        "the target was tapped before the scry"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(Vec::new()));
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 1,
        "the spell leaves hand and draws one after the scry"
    );

    // Casting with no target is legal: scry 1 and draw without tapping anything.
    let mut no_target = engine(726_015);
    inject_card_into_hand(&mut no_target, 0, "plunge_into_winter");
    let hand_before = no_target.state.players[0].hand.len();
    let slot = hand_index_for_card(&no_target, 0, "plunge_into_winter");
    semantic::accepted(&mut no_target, 0, &cast_spell(slot, vec![]));
    no_target.apply_command(0, &pass()).expect("p0 pass");
    let batch = no_target.apply_command(1, &pass()).expect("the scry parks");
    assert!(find_resolution_choice(&batch).is_some());
    semantic::accepted(&mut no_target, 0, &submit_resolution_choice(Vec::new()));
    assert_eq!(no_target.state.players[0].hand.len(), hand_before - 1 + 1);

    // If the target is already tapped, the spell still scrys and draws.
    let mut already = engine(726_035);
    let tapped = inject_creature_on_battlefield(&mut already, 1, "grizzly_bears");
    already
        .state
        .objects
        .get_mut(&tapped)
        .expect("object")
        .tapped = true;
    inject_card_into_hand(&mut already, 0, "plunge_into_winter");
    let hand_before = already.state.players[0].hand.len();
    let slot = hand_index_for_card(&already, 0, "plunge_into_winter");
    semantic::accepted(&mut already, 0, &cast_spell(slot, target_object(tapped)));
    already.apply_command(0, &pass()).expect("p0 pass");
    let batch = already.apply_command(1, &pass()).expect("the scry parks");
    assert!(find_resolution_choice(&batch).is_some());
    semantic::accepted(&mut already, 0, &submit_resolution_choice(Vec::new()));
    assert_eq!(already.state.players[0].hand.len(), hand_before - 1 + 1);

    // If a chosen target is illegal at resolution, the whole spell fizzles: no scry, no draw.
    let mut fizzle = engine(726_025);
    let target = inject_creature_on_battlefield(&mut fizzle, 1, "grizzly_bears");
    inject_card_into_hand(&mut fizzle, 0, "plunge_into_winter");
    let hand_before = fizzle.state.players[0].hand.len();
    let slot = hand_index_for_card(&fizzle, 0, "plunge_into_winter");
    semantic::accepted(&mut fizzle, 0, &cast_spell(slot, target_object(target)));
    fizzle.state.players[1]
        .battlefield
        .retain(|id| *id != target);
    fizzle.state.players[1].graveyard.push(target);
    fizzle.state.objects.get_mut(&target).expect("object").zone = Zone::Graveyard;
    *fizzle
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzle);
    assert_eq!(
        fizzle.state.players[0].hand.len(),
        hand_before - 1,
        "an illegal target fizzles the spell before the scry and draw"
    );
}

#[test]
fn issue_misc2_mind_spring() {
    let mut e = engine(726_006);
    inject_card_into_hand(&mut e, 0, "mind_spring");
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "mind_spring");
    semantic::accepted(&mut e, 0, &cast_spell_x(slot, vec![], 3));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 3,
        "X = 3 draws three cards"
    );
}
