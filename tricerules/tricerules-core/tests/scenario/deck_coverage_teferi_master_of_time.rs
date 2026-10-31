//! Teferi, Master of Time actual-card prepared-state scenarios.
//!
//! Strict normal `EngineSession` cast/+1 reachability is covered separately in `replay.rs`.

use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{AttachmentRecipient, Zone};
use tricerules_proto::ruled::v1 as rv1;

const TEFERI: &str = "teferi,_master_of_time";
const TEFERI_FACE: &str = "teferi_master_of_time";
const ORDINARY_PLANESWALKER: &str = "jace,_unraveler_of_secrets";
const CONTROLLER: i32 = 10;
const OPPONENT: i32 = 20;

fn setup(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    let definition = registry
        .get(TEFERI)
        .expect("Teferi, Master of Time is registered before actual-card scenarios run");
    assert_eq!(definition.id, TEFERI);
    assert_eq!(definition.name, "Teferi, Master of Time");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), TEFERI_FACE);
    assert_eq!(face.name, "Teferi, Master of Time");
    assert_eq!(face.mana_cost.to_string(), "{2}{U}{U}");
    assert_eq!(face.supertypes, vec!["Legendary".to_string()]);
    assert_eq!(
        face.types,
        vec!["Planeswalker".to_string(), "Teferi".to_string()]
    );
    assert_eq!(face.loyalty, Some(3));

    for card in [
        ORDINARY_PLANESWALKER,
        "grizzly_bears",
        "pacifism",
        "forest",
        "lightning_bolt",
    ] {
        registry
            .get(card)
            .unwrap_or_else(|| panic!("{card} must be registered for this fixture"));
    }

    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[CONTROLLER, OPPONENT],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player actual Teferi scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_teferi(engine: &mut GameEngine, seat: usize) -> u32 {
    let player = engine.state.players[seat].id;
    let source = inject_card_into_hand(engine, seat, TEFERI);
    give_mana(
        engine,
        player,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(engine, seat, TEFERI);
    engine
        .apply_scenario_command(player, &cast_spell(hand_index, vec![]))
        .expect("pay {2}{U}{U} for the registered Teferi");
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&source].card_id, TEFERI);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        3,
        "the actual registered face enters with its printed starting loyalty"
    );
    source
}

fn ability_info(engine: &mut GameEngine, source: u32, index: u32) -> rv1::AbilityInfo {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| {
            let Some(rv1::ruled_event::Ev::ZoneView(view)) = &event.ev else {
                return None;
            };
            view.per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source)
                .and_then(|object| {
                    object
                        .activated_abilities
                        .iter()
                        .find(|ability| ability.ability_index == index)
                })
                .cloned()
        })
        .expect("the engine publishes this registered activated-ability offer")
}

fn reject_without_mutation(engine: &mut GameEngine, player: i32, command: &RuledCommand) {
    let before = engine
        .diagnostic_snapshot()
        .expect("state snapshot before rejected actual-card command");
    assert!(engine.apply_scenario_command(player, command).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("state snapshot after rejected actual-card command"),
        before,
        "an illegal actual-card path does not pay loyalty or change state"
    );
}

#[test]
fn teferi_is_the_only_offturn_loyalty_source_and_plus_one_draws_then_discards() {
    let mut engine = setup(26_103_101);
    let teferi = cast_teferi(&mut engine, 0);
    let ordinary_pw = inject_permanent_on_battlefield(&mut engine, 0, ORDINARY_PLANESWALKER);
    engine
        .state
        .objects
        .get_mut(&ordinary_pw)
        .expect("ordinary registered planeswalker")
        .set_counter(CounterKind::Loyalty, 4);
    engine.initial_response_batch();

    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    assert!(engine.state.stack.is_empty());

    // The active player has priority first. Teferi's static is self-only, and it grants timing
    // permission only while its own controller receives priority on the opponent's turn.
    let wrong_controller = activate_ability_for(&engine, teferi, 0, vec![]);
    reject_without_mutation(&mut engine, OPPONENT, &wrong_controller);
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("opponent passes to Teferi's controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert!(ability_info(&mut engine, teferi, 0).activatable);
    assert!(
        !ability_info(&mut engine, ordinary_pw, 0).activatable,
        "Teferi's static does not grant instant timing to another planeswalker"
    );

    let loyalty_before = engine.state.objects[&teferi].counter_count(CounterKind::Loyalty);
    let player_index = engine
        .state
        .player_idx(CONTROLLER)
        .expect("Teferi controller");
    let hand_before = engine.state.players[player_index].hand.len();
    let library_before = engine.state.players[player_index].library.len();
    let draws_before = engine
        .state
        .turn_history
        .current
        .player(CONTROLLER)
        .cards_drawn;
    let activation = activate_ability_for(&engine, teferi, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("activate the actual Teferi +1 at instant timing");
    assert_eq!(
        engine.state.objects[&teferi].counter_count(CounterKind::Loyalty),
        loyalty_before + 1
    );
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .and_then(|item| item.source_permanent_id),
        Some(teferi),
        "the activation is on the stack before resolution"
    );

    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the actual +1 draws, then parks for its mandatory discard");
    assert_eq!(pending.deciding_player, CONTROLLER);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
    assert_eq!(
        engine.state.players[player_index].library.len(),
        library_before - 1
    );
    assert_eq!(
        engine.state.players[player_index].hand.len(),
        hand_before + 1
    );
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(CONTROLLER)
            .cards_drawn,
        draws_before + 1
    );
    let discarded = *pending
        .presentation
        .candidates
        .first()
        .expect("the draw leaves a legal private hand card to discard");

    // Submit the sole discard immediately. There is no intervening gameplay command between the
    // draw/choice prompt and the controller's answer.
    engine
        .apply_scenario_command(CONTROLLER, &submit_resolution_choice(vec![discarded]))
        .expect("choose exactly one card from Teferi's controller's hand");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[player_index].library.len(),
        library_before - 1
    );
    assert_eq!(engine.state.players[player_index].hand.len(), hand_before);

    // This permanent has already used a loyalty ability this turn; the normal ledger still applies.
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("opponent passes after the +1 resolves");
    assert!(!ability_info(&mut engine, teferi, 0).activatable);
    let repeated = activate_ability_for(&engine, teferi, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &repeated);
}

#[test]
fn teferi_minus_three_rejects_bad_targets_then_phases_and_returns_the_same_opponent_creature() {
    let mut engine = setup(26_103_102);
    let teferi = cast_teferi(&mut engine, 0);
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    // Keep a separate legal attacker on the active player's battlefield while the target is phased out,
    // so the shared turn helper traverses the normal combat declaration steps.
    let _opponent_turn_runner = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let attachment = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    engine
        .state
        .objects
        .get_mut(&opponent_creature)
        .expect("opponent creature")
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    // Prepared attachment fixture: this Pacifism is controlled by the creature's controller and
    // is attached to a legal creature. Attachment engine behavior is not the target of this case.
    engine
        .state
        .objects
        .get_mut(&attachment)
        .expect("registered Pacifism Aura")
        .attached_to = Some(AttachmentRecipient::Object(opponent_creature));
    engine.initial_response_batch();

    let own_target = activate_ability_for(&engine, teferi, 1, target_object(own_creature));
    reject_without_mutation(&mut engine, CONTROLLER, &own_target);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let noncreature_target = activate_ability_for(&engine, teferi, 1, target_object(land));
    reject_without_mutation(&mut engine, CONTROLLER, &noncreature_target);
    assert_eq!(
        engine.state.objects[&teferi].counter_count(CounterKind::Loyalty),
        3,
        "rejected target sets do not pay the loyalty cost"
    );

    let target_generation = engine
        .state
        .zone_change_generation
        .get(&opponent_creature)
        .copied()
        .unwrap_or_default();
    assert_eq!(
        target_generation, 0,
        "the prepared object begins in generation zero"
    );
    let counters_before = engine.state.objects[&opponent_creature].counters.clone();
    let attachment_recipient_before = engine.state.objects[&attachment].attached_to;
    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    advance_to_main1_from_game_start(&mut engine);
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("opponent gives Teferi's controller instant-speed priority");

    let activation = activate_ability_for(&engine, teferi, 1, target_object(opponent_creature));
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("the actual -3 targets an opponent's creature");
    assert_eq!(engine.state.objects[&teferi].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .and_then(|item| item.source_permanent_id),
        Some(teferi),
        "zero loyalty removes Teferi before the activated ability resolves"
    );

    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&opponent_creature].zone,
        Zone::Battlefield
    );
    assert!(engine.state.objects[&opponent_creature]
        .phase_status
        .is_some());
    assert!(engine.state.objects[&attachment].phase_status.is_some());
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&opponent_creature)
            .copied()
            .unwrap_or_default(),
        target_generation,
        "phasing preserves the target's object incarnation"
    );
    assert_eq!(
        engine.state.objects[&opponent_creature].counters,
        counters_before
    );
    assert_eq!(
        engine.state.objects[&attachment].attached_to,
        attachment_recipient_before
    );

    // Advance normally through the current opponent turn and the controller's turn, then through
    // the opponent's next untap. CR 702.26 phases this recipient in without a zone-change event;
    // this prepared scenario must assert that same ObjectId/generation, counters, and Aura link.
    end_active_turn(&mut engine, OPPONENT);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, CONTROLLER);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    assert_eq!(engine.state.objects[&opponent_creature].phase_status, None);
    assert_eq!(engine.state.objects[&attachment].phase_status, None);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&opponent_creature)
            .copied()
            .unwrap_or_default(),
        target_generation
    );
    assert_eq!(
        engine.state.objects[&opponent_creature].counters,
        counters_before
    );
    assert_eq!(
        engine.state.objects[&attachment].attached_to,
        Some(AttachmentRecipient::Object(opponent_creature))
    );
    assert!(engine.state.players[1]
        .battlefield
        .contains(&opponent_creature));
}

#[test]
fn teferi_minus_three_does_not_phase_out_a_target_removed_before_resolution() {
    let mut engine = setup(26_103_105);
    let teferi = cast_teferi(&mut engine, 0);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let bolt = inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    give_mana(
        &mut engine,
        OPPONENT,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let target_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);

    let activation = activate_ability_for(&engine, teferi, 1, target_object(target));
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("announce the legal opponent-creature target");
    assert_eq!(engine.state.objects[&teferi].zone, Zone::Graveyard);
    engine
        .apply_scenario_command(CONTROLLER, &pass())
        .expect("Teferi's controller passes priority with -3 on the stack");
    let bolt_index = hand_index_for_card(&engine, 1, "lightning_bolt");
    assert_eq!(engine.state.players[1].hand[bolt_index], bolt);
    engine
        .apply_scenario_command(OPPONENT, &cast_spell(bolt_index, target_object(target)))
        .expect("opponent responds by removing the announced target");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Teferi's ability remains below Bolt"
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_ne!(
        engine.state.zone_change_generation.get(&target).copied(),
        Some(target_generation),
        "the target left its announced battlefield incarnation"
    );

    // The remaining actual -3 resolves with no legal target and has no phase-out effect. The
    // B-kernel stale-incarnation cases remain the dedicated evidence for reusing an OID as a new
    // incarnation; this actual-card draft does not fabricate that lifecycle transition.
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.objects[&target].phase_status.is_none());
}

#[test]
fn teferi_minus_three_removes_an_attacker_from_combat_even_when_teferi_dies() {
    let mut engine = setup(26_103_103);
    let teferi = cast_teferi(&mut engine, 0);
    let attacker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    end_active_turn(&mut engine, CONTROLLER);
    advance_to_main1_from_game_start(&mut engine);
    engine
        .apply_scenario_command(OPPONENT, &primitive_yield())
        .expect("opponent begins combat");
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("active player passes into declare attackers");
    engine
        .apply_scenario_command(CONTROLLER, &pass())
        .expect("defending player passes into declare attackers");
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    let current_attack_offers = engine.initial_response_batch();
    let assignment = current_attack_offers.legal_by_player[&OPPONENT]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == attacker
                && assignment.defending_player_id == CONTROLLER
                && assignment.defender.as_ref().is_some_and(|defender| {
                    defender.kind == rv1::TargetRefKind::Player as i32
                        && defender.object_id == CONTROLLER as u32
                })
        })
        .cloned()
        .expect("the engine offers this attacker against Teferi's controller");
    engine
        .apply_scenario_command(
            OPPONENT,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::DeclareAttackers(
                    rv1::DeclareAttackers {
                        assignments: vec![assignment],
                    },
                )),
            },
        )
        .expect("the opponent creature attacks the Teferi controller");
    assert!(engine
        .state
        .combat
        .as_ref()
        .unwrap()
        .attacking
        .contains(&attacker));
    let attack = engine.state.combat.as_ref().unwrap().attack_assignments[&attacker];
    assert_eq!(attack.defending_player, CONTROLLER);
    assert!(matches!(
        attack.defender,
        tricerules_core::state::CombatDefenderTarget::Player(CONTROLLER)
    ));
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("opponent passes combat priority to Teferi's controller");

    let activation = activate_ability_for(&engine, teferi, 1, target_object(attacker));
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("Teferi can use its own -3 during the opponent's combat");
    assert_eq!(engine.state.objects[&teferi].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);
    assert!(engine.state.objects[&attacker].phase_status.is_some());
    assert!(
        !engine
            .state
            .combat
            .as_ref()
            .expect("combat continues after the attacker phases out")
            .attacking
            .contains(&attacker),
        "phasing out removes the attacker from combat and it does not rejoin"
    );
}

#[test]
fn teferi_minus_ten_schedules_exactly_two_controller_turns_before_rotation() {
    let mut engine = setup(26_103_104);
    let teferi = cast_teferi(&mut engine, 0);
    // Fixture-only loyalty setup makes the actual registered -10 selectable. Its activation cost,
    // source death, stack continuation, and extra-turn result remain engine-driven.
    engine
        .state
        .objects
        .get_mut(&teferi)
        .expect("actual Teferi permanent")
        .set_counter(CounterKind::Loyalty, 10);
    let activation = activate_ability_for(&engine, teferi, 2, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("pay the actual -10 loyalty cost");
    assert_eq!(engine.state.objects[&teferi].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0].source_permanent_id,
        Some(teferi),
        "the activated ultimate remains on the stack after its source leaves"
    );
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.active_player_id(), CONTROLLER);

    // A ready attacker keeps the prepared turn walk on the normal combat steps while checking the
    // queued-turn insertion order. It does not alter Teferi's ability or its scheduled turns.
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(
        engine.state.active_player_id(),
        CONTROLLER,
        "first extra turn"
    );
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(
        engine.state.active_player_id(),
        CONTROLLER,
        "second extra turn"
    );
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(
        engine.state.active_player_id(),
        OPPONENT,
        "ordinary rotation resumes after exactly two extra turns"
    );
}

// Strict no-development-command EngineSession cast/+1 proof:
// see `tricerules-server/src/replay.rs::teferi_master_of_time_casts_and_resolves_plus_one_from_normal_offers`.
