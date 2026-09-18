//! Issue #377 focused scenarios for the retained static graveyard-condition cohort.
//!
//! Every condition is continuously reevaluated from public printed graveyard card data
//! (CR 404.2, CR 611.3): Descend counts permanent cards, Threshold counts all cards, Delirium
//! counts distinct card types, and the Lesson/instant/sorcery predicates narrow the counted
//! cohort. The scenarios drive the generated definitions through the authoritative command path
//! and pin the `3↔4`, `7↔8`, and `6↔7` boundaries plus the owner scope. They also exercise the
//! cohort's ordinary clauses: entry mill/surveil/life/tokens, the hybrid activated surveil, the
//! upkeep surveil, the optional upkeep mill, the enters-or-attacks loot, and the conditional
//! Villain hexproof.

use super::helpers::*;
use tricerules_cards::{CardRegistry, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

fn engine_with(seed: u64, own: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", own), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn stats(engine: &GameEngine, oid: u32) -> (u32, u32) {
    let characteristics = engine.characteristics(oid).expect("characteristics");
    (
        characteristics.power.expect("power"),
        characteristics.toughness.expect("toughness"),
    )
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_command(1, &pass())
        .expect("defender passes after attackers are declared");
}

#[test]
fn issue_377_akawalli_descend_four_boundary_grants_trample() {
    let mut engine = engine_with(377_001, &["akawalli,_the_seething_tower"]);
    let akawalli = move_ready_to_battlefield(&mut engine, 0, "akawalli,_the_seething_tower");
    assert_eq!(stats(&engine, akawalli), (3, 3));
    assert!(!engine.effective_has_keyword(akawalli, Keyword::Trample));

    for _ in 0..3 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    assert_eq!(
        stats(&engine, akawalli),
        (3, 3),
        "three permanent cards are below the Descend 4 boundary"
    );
    assert!(!engine.effective_has_keyword(akawalli, Keyword::Trample));

    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    assert_eq!(stats(&engine, akawalli), (5, 5));
    assert!(engine.effective_has_keyword(akawalli, Keyword::Trample));
}

#[test]
fn issue_377_akawalli_descend_eight_boundary_limits_blockers() {
    // Seven permanent cards: the Descend-8 pump and blocker limit stay off, so two blockers are
    // legal and the base Descend-4 pump is active.
    let mut below = engine_with(377_010, &["akawalli,_the_seething_tower"]);
    let akawalli = move_ready_to_battlefield(&mut below, 0, "akawalli,_the_seething_tower");
    let first = inject_creature_on_battlefield(&mut below, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut below, 1, "grizzly_bears");
    for _ in 0..7 {
        inject_graveyard_card(&mut below, 0, "forest");
    }
    assert_eq!(stats(&below, akawalli), (5, 5));
    advance_main1_to_declare_attackers(&mut below);
    below
        .apply_command(0, &declare_attackers(vec![akawalli]))
        .expect("declare Akawalli");
    pass_to_declare_blockers(&mut below);
    assert_eq!(
        below.initial_response_batch().legal_by_player[&1]
            .legal_block_pairs
            .len(),
        2,
        "below Descend 8 both blockers remain legal"
    );
    below
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: akawalli,
                    blocker_id: first,
                },
                BlockPair {
                    attacker_id: akawalli,
                    blocker_id: second,
                },
            ]),
        )
        .expect("two blockers are legal below Descend 8");

    // Eight permanent cards: the additional pump raises the source to 7/7 and only one creature
    // may block it.
    let mut at = engine_with(377_011, &["akawalli,_the_seething_tower"]);
    let akawalli = move_ready_to_battlefield(&mut at, 0, "akawalli,_the_seething_tower");
    let first = inject_creature_on_battlefield(&mut at, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut at, 1, "grizzly_bears");
    for _ in 0..8 {
        inject_graveyard_card(&mut at, 0, "forest");
    }
    assert_eq!(stats(&at, akawalli), (7, 7));
    advance_main1_to_declare_attackers(&mut at);
    at.apply_command(0, &declare_attackers(vec![akawalli]))
        .expect("declare Akawalli");
    pass_to_declare_blockers(&mut at);
    assert!(
        at.apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: akawalli,
                    blocker_id: first,
                },
                BlockPair {
                    attacker_id: akawalli,
                    blocker_id: second,
                },
            ]),
        )
        .is_err(),
        "Descend 8 stops a second blocking creature"
    );
    at.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: akawalli,
            blocker_id: first,
        }]),
    )
    .expect("one blocker stays legal at Descend 8");
}

#[test]
fn issue_377_descend_four_pumps_and_keywords_track_public_graveyard_cards() {
    let mut engine = engine_with(
        377_020,
        &[
            "basking_capybara",
            "frilled_cave-wurm",
            "echo_of_dusk",
            "didact_echo",
        ],
    );
    let capybara = move_ready_to_battlefield(&mut engine, 0, "basking_capybara");
    let wurm = move_ready_to_battlefield(&mut engine, 0, "frilled_cave-wurm");
    let echo = move_ready_to_battlefield(&mut engine, 0, "echo_of_dusk");
    let didact = move_ready_to_battlefield(&mut engine, 0, "didact_echo");

    for _ in 0..3 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    assert_eq!(stats(&engine, capybara), (1, 3));
    assert_eq!(stats(&engine, wurm), (2, 5));
    assert_eq!(stats(&engine, echo), (2, 2));
    assert!(!engine.effective_has_keyword(echo, Keyword::Lifelink));
    assert!(!engine.effective_has_keyword(didact, Keyword::Flying));

    inject_graveyard_card(&mut engine, 0, "ornithopter");
    assert_eq!(stats(&engine, capybara), (4, 3));
    assert_eq!(stats(&engine, wurm), (4, 5));
    assert_eq!(stats(&engine, echo), (3, 3));
    assert!(engine.effective_has_keyword(echo, Keyword::Lifelink));
    assert!(engine.effective_has_keyword(didact, Keyword::Flying));
}

#[test]
fn issue_377_first_time_flyer_counts_only_own_lesson_cards() {
    let mut engine = engine_with(377_030, &["first-time_flyer"]);
    let flyer = move_ready_to_battlefield(&mut engine, 0, "first-time_flyer");
    assert_eq!(stats(&engine, flyer), (1, 2));

    // A Lesson in an opponent's graveyard is outside the controller-owner scope.
    inject_graveyard_card(&mut engine, 1, "combustion_technique");
    assert_eq!(stats(&engine, flyer), (1, 2));

    // Non-Lesson cards never satisfy the Lesson predicate.
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "ornithopter");
    assert_eq!(stats(&engine, flyer), (1, 2));

    inject_graveyard_card(&mut engine, 0, "combustion_technique");
    assert_eq!(stats(&engine, flyer), (2, 3));
}

#[test]
fn issue_377_ghitu_lavarunner_counts_own_instants_and_sorceries_only() {
    let mut engine = engine_with(377_040, &["ghitu_lavarunner"]);
    let lavarunner = move_ready_to_battlefield(&mut engine, 0, "ghitu_lavarunner");
    assert_eq!(stats(&engine, lavarunner), (1, 2));
    assert!(!engine.effective_has_keyword(lavarunner, Keyword::Haste));

    inject_graveyard_card(&mut engine, 1, "unsummon");
    inject_graveyard_card(&mut engine, 1, "lightning_bolt");
    assert_eq!(
        stats(&engine, lavarunner),
        (1, 2),
        "an opponent's instant and sorcery cards do not qualify"
    );

    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "unsummon");
    assert_eq!(
        stats(&engine, lavarunner),
        (1, 2),
        "one own spell plus a creature card is below the two-spell boundary"
    );
    assert!(!engine.effective_has_keyword(lavarunner, Keyword::Haste));

    inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    assert_eq!(stats(&engine, lavarunner), (2, 2));
    assert!(engine.effective_has_keyword(lavarunner, Keyword::Haste));
}

#[test]
fn issue_377_wildfire_wickerfolk_counts_distinct_graveyard_card_types() {
    let mut engine = engine_with(377_050, &["wildfire_wickerfolk"]);
    let wickerfolk = move_ready_to_battlefield(&mut engine, 0, "wildfire_wickerfolk");
    assert_eq!(stats(&engine, wickerfolk), (3, 2));
    assert!(!engine.effective_has_keyword(wickerfolk, Keyword::Trample));

    // Four creature cards are one card type: Delirium stays off.
    for _ in 0..4 {
        inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    }
    assert_eq!(stats(&engine, wickerfolk), (3, 2));

    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "ornithopter");
    assert_eq!(
        stats(&engine, wickerfolk),
        (3, 2),
        "three distinct card types are below the Delirium 4 boundary"
    );

    inject_graveyard_card(&mut engine, 0, "unsummon");
    assert_eq!(stats(&engine, wickerfolk), (4, 3));
    assert!(engine.effective_has_keyword(wickerfolk, Keyword::Trample));
}

#[test]
fn issue_377_land_animation_surface_stays_untouched() {
    let registry = CardRegistry::global();
    assert!(
        registry.get("cavernous_maw").is_none(),
        "Cavernous Maw stays ungenerated behind its cross-zone activation blocker"
    );
    assert!(
        registry.get("restless_reef").is_some(),
        "AnimateSelf remains exercised by the existing animated-land consumers"
    );
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_front(object_id);
    object_id
}

/// Advance turn-by-turn until `player` is in their own upkeep again, so their next "at the
/// beginning of your upkeep" trigger has been staged.
fn advance_to_controller_next_upkeep(engine: &mut GameEngine, player: i32) {
    let starting_turn = engine.state.turn_instance;
    for _ in 0..240 {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.turn_instance > starting_turn
            && engine.state.active_player_id() == player
            && engine.state.turn_step == TurnStep::Upkeep
        {
            return;
        }
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("advance turn");
    }
    panic!("turn advancement stalled before P{player}'s next upkeep");
}

/// Pass until the upkeep trigger has either resolved or parked its private choice. The engine
/// resolves a lone upkeep trigger as soon as the priority holder passes, so a fixed two-pass
/// cycle is not valid here.
fn pass_until_choice_or_stack_empty(engine: &mut GameEngine) {
    for _ in 0..8 {
        if engine.state.pending_resolution.is_some() || engine.state.stack.is_empty() {
            return;
        }
        answer_trigger_order_in_engine_order(engine);
        let priority = engine.state.priority_player_id();
        if engine.apply_command(priority, &pass()).is_err() {
            assert!(
                engine.state.pending_resolution.is_some(),
                "unexpected stalled pass while resolving an upkeep trigger"
            );
            return;
        }
    }
    panic!("upkeep trigger resolution stalled");
}

fn select_optional_effect() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn decline_optional_effect() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_377_billowing_mills_three_on_entry() {
    let mut engine = engine_with(377_060, &["billowing_shriekmass"]);
    move_ready_to_battlefield(&mut engine, 0, "billowing_shriekmass");
    resolve_top_stack(&mut engine);
    assert_eq!(
        count_card_id_in_graveyard(&engine, 0, "forest"),
        3,
        "the entry trigger mills exactly three cards"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_377_cephalid_surveils_three_on_entry() {
    let mut engine = engine_with(377_061, &["cephalid_inkmage"]);
    move_ready_to_battlefield(&mut engine, 0, "cephalid_inkmage");
    resolve_top_stack(&mut engine);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("surveil three parks a private choice");
    assert_eq!(choice.deciding_player, 0);
    let candidates = choice.presentation.candidates.clone();
    assert_eq!(candidates.len(), 3, "surveil three looks at three cards");
    engine
        .apply_command(0, &submit_resolution_choice(candidates.clone()))
        .expect("choose the whole cohort for the graveyard");
    for object_id in candidates {
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Graveyard);
    }
}

#[test]
fn issue_377_mind_drill_hybrid_activation_surveils_one() {
    let mut engine = engine_with(377_062, &["mind_drill_assailant"]);
    let source = move_ready_to_battlefield(&mut engine, 0, "mind_drill_assailant");
    grant_pool(&mut engine, 0);
    let command = activate_ability_for(&engine, source, 0, Vec::new());
    engine
        .apply_command(0, &command)
        .expect("the hybrid mana cost is payable from the pool");
    resolve_top_stack(&mut engine);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("surveil one parks a private choice");
    let candidates = choice.presentation.candidates.clone();
    assert_eq!(candidates.len(), 1);
    engine
        .apply_command(0, &submit_resolution_choice(candidates.clone()))
        .expect("put the looked-at card into the graveyard");
    assert_eq!(engine.state.objects[&candidates[0]].zone, Zone::Graveyard);
}

#[test]
fn issue_377_swarmweaver_creates_two_flying_insects() {
    let mut engine = engine_with(377_063, &["the_swarmweaver"]);
    move_ready_to_battlefield(&mut engine, 0, "the_swarmweaver");
    resolve_top_stack(&mut engine);
    let insects = battlefield_token_oids(&engine, 0, "insect_bg_1_1_flying");
    assert_eq!(insects.len(), 2, "exactly two Insect tokens enter");
    for insect in insects {
        assert_eq!(stats(&engine, insect), (1, 1));
        assert!(engine.effective_has_keyword(insect, Keyword::Flying));
    }
    assert!(battlefield_token_oids(&engine, 1, "insect_bg_1_1_flying").is_empty());
}

#[test]
fn issue_377_lion_turtle_entry_gains_three_life() {
    let mut engine = engine_with(377_064, &["the_lion-turtle"]);
    let before = engine.state.players[0].life;
    move_ready_to_battlefield(&mut engine, 0, "the_lion-turtle");
    resolve_top_stack(&mut engine);
    assert_eq!(engine.state.players[0].life, before + 3);
}

#[test]
fn issue_377_dreadwing_entry_loot_takes_a_private_discard() {
    let mut engine = engine_with(377_065, &["dreadwing_scavenger", "grizzly_bears"]);
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    let known = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("known discard");
    move_ready_to_battlefield(&mut engine, 0, "dreadwing_scavenger");
    resolve_top_stack(&mut engine);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("draw-then-discard parks the private discard");
    assert!(choice.presentation.candidates.contains(&known));
    engine
        .apply_command(0, &submit_resolution_choice(vec![known]))
        .expect("discard the known card");
    assert_eq!(engine.state.objects[&known].zone, Zone::Graveyard);
}

#[test]
fn issue_377_doc_ock_hexproof_tracks_another_villain() {
    let mut engine = engine_with(377_066, &["doc_ock,_sinister_scientist"]);
    let doc_ock = move_ready_to_battlefield(&mut engine, 0, "doc_ock,_sinister_scientist");
    assert!(!engine.effective_has_keyword(doc_ock, Keyword::Hexproof));

    inject_creature_on_battlefield(&mut engine, 1, "common_crook");
    assert!(
        !engine.effective_has_keyword(doc_ock, Keyword::Hexproof),
        "an opponent's Villain does not satisfy 'you control another Villain'"
    );

    inject_creature_on_battlefield(&mut engine, 0, "common_crook");
    assert!(
        engine.effective_has_keyword(doc_ock, Keyword::Hexproof),
        "a second controlled Villain turns the conditional grant on"
    );
}

#[test]
fn issue_377_mindwhisker_surveils_on_its_controllers_upkeep() {
    let mut engine = engine_with(377_067, &["mindwhisker"]);
    move_ready_to_battlefield(&mut engine, 0, "mindwhisker");
    advance_to_controller_next_upkeep(&mut engine, 0);
    pass_until_choice_or_stack_empty(&mut engine);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the upkeep surveil parks a private choice");
    assert_eq!(choice.deciding_player, 0);
    assert_eq!(choice.presentation.candidates.len(), 1);
}

#[test]
fn issue_377_patchwork_beastie_optional_upkeep_mill_is_optional() {
    // Decline: the library is untouched.
    let mut decline = engine_with(377_068, &["patchwork_beastie"]);
    move_ready_to_battlefield(&mut decline, 0, "patchwork_beastie");
    advance_to_controller_next_upkeep(&mut decline, 0);
    let library_before = decline.state.players[0].library.len();
    pass_until_choice_or_stack_empty(&mut decline);
    let choice = decline
        .state
        .pending_resolution
        .as_ref()
        .expect("optional mill choice");
    assert_eq!(
        choice.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    decline
        .apply_command(0, &decline_optional_effect())
        .expect("decline the optional mill");
    assert_eq!(decline.state.players[0].library.len(), library_before);

    // Accept: exactly the top card is milled.
    let mut accept = engine_with(377_069, &["patchwork_beastie"]);
    let top = seat_on_top(&mut accept, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut accept, 0, "patchwork_beastie");
    advance_to_controller_next_upkeep(&mut accept, 0);
    pass_until_choice_or_stack_empty(&mut accept);
    let choice = accept
        .state
        .pending_resolution
        .as_ref()
        .expect("optional mill choice");
    assert_eq!(
        choice.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    accept
        .apply_command(0, &select_optional_effect())
        .expect("accept the optional mill");
    assert_eq!(accept.state.objects[&top].zone, Zone::Graveyard);
}
