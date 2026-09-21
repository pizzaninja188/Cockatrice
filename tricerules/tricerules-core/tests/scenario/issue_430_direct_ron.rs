//! Issue #430 — reviewed direct-RON scenarios for Sunset Saboteur.
//!
//! Exact Scryfall record and `rulings_uri` were fetched 2026-09-20 against the pinned snapshot
//! identity `c3731735-2b78-4a15-bff4-4d8559401f68`. Sunset Saboteur prints `Menace`,
//! `Ward—Discard a card.`, and `Whenever this creature attacks, put a +1/+1 counter on target
//! creature an opponent controls.`; it has no rulings.
//!
//! CR 702.111 (menace), 702.21a-b (Ward observes an opponent's spell or ability and its
//! controller is the payer; paying preserves the stack object, declining or being unable counters
//! it), 508.1m/508.3a and 603.2c (attack declaration triggers and target publication), 115.1 and
//! 608.2b (targeting and revalidation), 701.5 (counter), and 122.1 (counters) govern. The
//! discard-choice privacy, hand movement, and stale-candidate handling reuse the Ward discard
//! contracts established by Spectral Snatcher (issue #103) with this card's own definition.

use super::helpers::*;
use tricerules_core::{EngineError, Zone};
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, ResolutionChoiceDecision, TargetRefKind};

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

/// A two-player game with empty specials and both pools refilled. Cards are injected so the exact
/// object under test is the one cast or targeted.
fn saboteur_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("swamp", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

/// A two-player game advanced to the declare-attackers step with both pools refilled.
fn combat_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("swamp", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn empty_hand_to_graveyard(engine: &mut GameEngine, player: usize) {
    for card in std::mem::take(&mut engine.state.players[player].hand) {
        engine.state.players[player].graveyard.push(card);
        engine.state.objects.get_mut(&card).expect("hand card").zone = Zone::Graveyard;
        *engine.state.zone_change_generation.entry(card).or_default() += 1;
    }
}

#[test]
fn issue_430_ward_pays_a_discard_to_preserve_an_opposing_spell_and_ability() {
    // Opposing spell: P1 controls Sunset Saboteur; P0's Unsummon makes P0 the Ward payer.
    let mut spell_engine = saboteur_engine(430_001);
    let saboteur = inject_creature_on_battlefield(&mut spell_engine, 1, "sunset_saboteur");
    let discard = inject_card_into_hand(&mut spell_engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut spell_engine, 0, "unsummon");
    grant_pool(&mut spell_engine, 0);
    let slot = hand_index_for_card(&spell_engine, 0, "unsummon");
    let spell_id = spell_engine.state.players[0].hand[slot];
    semantic::accepted(
        &mut spell_engine,
        0,
        &cast_spell(slot, target_object(saboteur)),
    );
    assert_eq!(spell_engine.state.stack.len(), 2, "Ward above Unsummon");
    let ward = spell_engine.state.stack.last().expect("Ward trigger");
    assert!(ward.is_triggered);
    assert_eq!(ward.controller, 1, "the Saboteur's controller owns Ward");

    pass_both_players(&mut spell_engine);
    let pending = spell_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward discard payment");
    assert_eq!(
        pending.deciding_player, 0,
        "the targeting spell's controller pays Ward (CR 702.21b)"
    );
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert!(pending.presentation.candidates.contains(&discard));

    semantic::accepted(
        &mut spell_engine,
        0,
        &submit_resolution_choice(vec![discard]),
    );
    assert_eq!(spell_engine.state.objects[&discard].zone, Zone::Graveyard);
    assert!(spell_engine
        .state
        .stack
        .iter()
        .any(|item| item.id == spell_id));
    pass_both_players(&mut spell_engine);
    assert_eq!(spell_engine.state.objects[&saboteur].zone, Zone::Hand);
    assert!(spell_engine.state.players[1].hand.contains(&saboteur));

    // Opposing activated ability: P0's Prodigal Sorcerer pings P1's Saboteur.
    let mut ability_engine = saboteur_engine(430_002);
    let saboteur = inject_creature_on_battlefield(&mut ability_engine, 1, "sunset_saboteur");
    let discard = inject_card_into_hand(&mut ability_engine, 0, "grizzly_bears");
    let sorcerer = inject_creature_on_battlefield(&mut ability_engine, 0, "prodigal_sorcerer");
    grant_pool(&mut ability_engine, 0);
    semantic::accepted(
        &mut ability_engine,
        0,
        &activate_ability(sorcerer, 0, target_object(saboteur)),
    );
    assert_eq!(
        ability_engine.state.stack.len(),
        2,
        "Ward above the ping ability"
    );
    let ability_id = ability_engine.state.stack[0].id;
    pass_both_players(&mut ability_engine);
    let pending = ability_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward discard payment");
    assert_eq!(pending.deciding_player, 0);
    semantic::accepted(
        &mut ability_engine,
        0,
        &submit_resolution_choice(vec![discard]),
    );
    assert!(
        ability_engine
            .state
            .stack
            .iter()
            .any(|item| item.id == ability_id),
        "paying Ward preserves the exact targeting ability"
    );
    resolve_entire_stack_two_player(&mut ability_engine);
    assert_eq!(
        ability_engine.state.objects[&saboteur].damage, 1,
        "the un-countered ping still resolves against the Saboteur"
    );
    assert!(!ability_engine
        .state
        .stack
        .iter()
        .any(|item| item.id == ability_id));
}

#[test]
fn issue_430_ward_decline_and_unpayable_discard_counter_the_targeting_object() {
    // Declining discards nothing and counters Unsummon.
    let mut decline = saboteur_engine(430_010);
    let saboteur = inject_creature_on_battlefield(&mut decline, 1, "sunset_saboteur");
    inject_card_into_hand(&mut decline, 0, "grizzly_bears");
    inject_card_into_hand(&mut decline, 0, "unsummon");
    grant_pool(&mut decline, 0);
    let slot = hand_index_for_card(&decline, 0, "unsummon");
    let spell_id = decline.state.players[0].hand[slot];
    semantic::accepted(&mut decline, 0, &cast_spell(slot, target_object(saboteur)));
    pass_both_players(&mut decline);
    assert!(decline.state.pending_resolution.is_some());
    semantic::accepted(
        &mut decline,
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    );
    assert!(!decline.state.stack.iter().any(|item| item.id == spell_id));
    assert_eq!(decline.state.objects[&spell_id].zone, Zone::Graveyard);
    assert_eq!(decline.state.objects[&saboteur].zone, Zone::Battlefield);

    // Being unable to discard counters automatically with no prompt.
    let mut unable = saboteur_engine(430_011);
    let saboteur = inject_creature_on_battlefield(&mut unable, 1, "sunset_saboteur");
    inject_card_into_hand(&mut unable, 0, "unsummon");
    grant_pool(&mut unable, 0);
    let slot = hand_index_for_card(&unable, 0, "unsummon");
    let spell_id = unable.state.players[0].hand[slot];
    semantic::accepted(&mut unable, 0, &cast_spell(slot, target_object(saboteur)));
    empty_hand_to_graveyard(&mut unable, 0);
    pass_both_players(&mut unable);
    assert!(unable.state.pending_resolution.is_none());
    assert!(!unable.state.stack.iter().any(|item| item.id == spell_id));
    assert_eq!(unable.state.objects[&spell_id].zone, Zone::Graveyard);
    assert_eq!(unable.state.objects[&saboteur].zone, Zone::Battlefield);
}

#[test]
fn issue_430_ward_ignores_its_controllers_own_targeting() {
    // P0 controls Sunset Saboteur and targets it with its own Unsummon: no Ward trigger.
    let mut engine = saboteur_engine(430_020);
    let saboteur = inject_creature_on_battlefield(&mut engine, 0, "sunset_saboteur");
    inject_card_into_hand(&mut engine, 0, "unsummon");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(saboteur)));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "own targeting triggers no Ward"
    );
    assert!(engine.state.pending_trigger_order.is_none());
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&saboteur].zone, Zone::Hand);
}

#[test]
fn issue_430_ward_private_discard_rejects_a_stale_card_and_moves_the_selected_physical_card() {
    let mut engine = saboteur_engine(430_030);
    let saboteur = inject_creature_on_battlefield(&mut engine, 1, "sunset_saboteur");
    let discard = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "unsummon");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[slot];
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(saboteur)));
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward discard payment");
    assert_eq!(pending.deciding_player, 0, "only the payer is authorized");
    assert!(pending.presentation.candidates.contains(&discard));
    let candidates = pending.presentation.candidates.clone();

    // Only the payer may answer the private choice: another seat's submission is rejected
    // without clearing it or touching the stack.
    let unauthorized = engine
        .apply_command(1, &submit_resolution_choice(vec![discard]))
        .expect_err("only the Ward payer may choose the discard");
    assert!(
        matches!(&unauthorized, EngineError::Illegal(reason) if reason.contains("not your resolution choice")),
        "expected an authorization rejection, got {unauthorized:?}"
    );
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));

    // A stale physical card is rejected without clearing the choice or the stack object.
    engine.state.players[0].hand.retain(|id| *id != discard);
    engine.state.players[0].graveyard.push(discard);
    engine.state.objects.get_mut(&discard).expect("card").zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(discard)
        .or_default() += 1;
    let error = engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .expect_err("stale physical card must be rejected");
    assert!(matches!(error, EngineError::Illegal(_)));
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));

    // Another card that was already an authorized candidate pays and moves to its owner's
    // graveyard.
    let replacement = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|id| candidates.contains(id))
        .expect("a legal hand candidate remains");
    semantic::accepted(&mut engine, 0, &submit_resolution_choice(vec![replacement]));
    assert_eq!(engine.state.objects[&replacement].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&replacement));
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&saboteur].zone, Zone::Hand);
}

#[test]
fn issue_430_attack_trigger_places_one_counter_on_a_legal_opponent_creature_only() {
    let mut engine = combat_engine(430_040);
    let saboteur = inject_creature_on_battlefield(&mut engine, 0, "sunset_saboteur");
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let unrelated = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    engine
        .apply_command(0, &declare_attackers(vec![saboteur]))
        .expect("declare Sunset Saboteur as an attacker");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(
        engine.state.stack.is_empty(),
        "trigger waits for its target"
    );

    assert!(
        engine
            .apply_command(0, &choose_trigger_target(own))
            .is_err(),
        "a creature the controller controls is outside the opponent filter"
    );
    assert!(
        engine
            .apply_command(0, &choose_trigger_target(saboteur))
            .is_err(),
        "the attacking source is not controlled by an opponent"
    );
    engine
        .apply_command(0, &choose_trigger_target(opposing))
        .expect("choose an opponent-controlled creature");
    assert_eq!(engine.state.pending_triggers.len(), 0);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(opposing), Some(3));
    assert_eq!(engine.effective_toughness(opposing), Some(3));
    assert_eq!(engine.effective_power(unrelated), Some(2));
    assert_eq!(engine.effective_power(own), Some(2));

    // A stale chosen target is revalidated away at resolution (CR 608.2b).
    let mut stale = combat_engine(430_041);
    let saboteur = inject_creature_on_battlefield(&mut stale, 0, "sunset_saboteur");
    let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    let survivor = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    stale
        .apply_command(0, &declare_attackers(vec![saboteur]))
        .expect("declare the attacker");
    stale
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose the initially legal target");
    stale.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    stale.state.players[1].hand.push(target);
    stale.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *stale
        .state
        .zone_change_generation
        .entry(target)
        .or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&target].zone, Zone::Hand);
    assert!(
        stale.state.objects[&target].counters.is_empty(),
        "a stale target receives no counter"
    );
    assert_eq!(stale.effective_power(survivor), Some(2));
}
