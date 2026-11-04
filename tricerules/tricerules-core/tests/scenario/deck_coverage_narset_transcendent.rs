//! Actual registered Narset Transcendent cast and private +1 library-look scenarios.
//!
//! Mana, opening state, and the top-card fixtures are prepared directly. The Narset definition,
//! cast, loyalty activation, stack resolution, and choice handling use the global registry and
//! ordinary engine commands. These core scenarios do not establish normal wire/native or GUI
//! acceptance.

use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::ruled_event::Ev;

const NARSET: &str = "narset_transcendent";
const NARSET_NAME: &str = "Narset Transcendent";
const CONTROLLER: i32 = 17;
const OPPONENT: i32 = 29;

fn game_at_upkeep(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    let definition = registry
        .get(NARSET)
        .expect("Narset Transcendent is in the global card registry");
    assert_eq!(definition.id, NARSET);
    assert_eq!(definition.name, NARSET_NAME);

    let deck = deck_with("island", &[]);
    GameEngine::new(
        registry,
        seed,
        &[CONTROLLER, OPPONENT],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("create a prepared two-player Narset scenario")
}

fn game(seed: u64) -> GameEngine {
    let mut engine = game_at_upkeep(seed);
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn full_snapshot(engine: &GameEngine) -> serde_json::Value {
    engine
        .diagnostic_snapshot()
        .expect("complete server-only scenario snapshot")
}

fn spell_announcement(cast: rv1::CastSpell) -> rv1::SpellCastAnnouncement {
    rv1::SpellCastAnnouncement {
        as_foretold_source: cast.as_foretold_source,
        targets: cast.targets,
        x_value: cast.x_value,
        flex_payments: cast.flex_payments,
        face_index: cast.face_index,
        selected_modes: cast.selected_modes,
        source: cast.source,
        prototyped: cast.prototyped,
        cost_selections: cast.cost_selections,
        cast_cost_group_selections: cast.cast_cost_group_selections,
        cast_method: cast.cast_method,
        casting_permission_id: cast.casting_permission_id,
    }
}

fn begin_narset_cast(engine: &mut GameEngine, narset: u32) -> u64 {
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    let cast = cast_spell(hand_index_for_card(engine, 0, NARSET), vec![]);
    let Some(rv1::ruled_command::Cmd::CastSpell(cast)) = cast.cmd else {
        unreachable!("the helper builds a hand-cast announcement")
    };
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
            rv1::BeginSpellCast {
                announcement: Some(spell_announcement(cast)),
            },
        )),
    };
    engine
        .apply_scenario_command(CONTROLLER, &command)
        .expect("Begin reserves the actual Narset card before payment");

    let pending = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("actual Narset cast is awaiting payment");
    assert_eq!(pending.reserved_object_id, narset);
    assert_eq!(pending.original_zone, Zone::Hand);
    assert_eq!(pending.locked_total_cost, "{2}{W}{U}");
    assert_eq!(engine.state.objects[&narset].zone, Zone::Stack);
    assert!(!engine.state.players[0].hand.contains(&narset));
    pending.transaction_id
}

fn current_spell_payment(engine: &mut GameEngine) -> (u64, u64) {
    let batch = engine.initial_response_batch();
    let pending = batch
        .legal_by_player
        .get(&CONTROLLER)
        .and_then(|actions| actions.pending_spell_cast.as_ref())
        .expect("current legal actions publish the pending Narset payment");
    let payment = pending
        .payment_preview
        .as_ref()
        .expect("pending Narset cast publishes its payment preview");
    assert_eq!(payment.total_cost, "{2}{W}{U}");
    (pending.transaction_id, payment.revision)
}

fn narset_spell_commit(
    engine: &GameEngine,
    transaction_id: u64,
    revision: u64,
    mana: rv1::PaymentMana,
) -> rv1::CommitSpellCast {
    let pending = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("current Narset payment has a reserved spell");
    assert_eq!(pending.transaction_id, transaction_id);
    assert_eq!(engine.state.command_index, revision);
    let reserved_object_id = pending.reserved_object_id;
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&reserved_object_id)
        .copied()
        .unwrap_or(0);
    rv1::CommitSpellCast {
        transaction_id,
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: revision,
            source: Some(rv1::CostObjectRef {
                object_id: reserved_object_id,
                zone_change_generation: source_generation,
            }),
            mana: Some(mana),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn pay_narset_cast(engine: &mut GameEngine, transaction_id: u64, mana: rv1::PaymentMana) {
    let (current_transaction, revision) = current_spell_payment(engine);
    assert_eq!(current_transaction, transaction_id);
    let proposal = narset_spell_commit(engine, transaction_id, revision, mana);
    let preview = engine.preview_payment_scenario(
        CONTROLLER,
        &rv1::PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(proposal),
            ..Default::default()
        },
    );
    assert!(
        preview.valid && preview.complete,
        "Narset payment: {preview:?}"
    );
    assert_eq!(preview.total_cost, "{2}{W}{U}");
    let selection = preview
        .selection
        .expect("complete preview returns the exact payment selection");
    engine
        .apply_scenario_command(
            CONTROLLER,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id,
                        payment: Some(selection),
                        restricted_mana: preview.restricted_mana,
                    },
                )),
            },
        )
        .expect("commit the complete current payment");
}

fn reject_narset_cast_payment(
    seed: u64,
    pool: ManaGift,
    proposed_mana: rv1::PaymentMana,
    label: &str,
) {
    let mut engine = game(seed);
    let narset = inject_card_into_hand(&mut engine, 0, NARSET);
    give_mana(&mut engine, CONTROLLER, pool);
    let transaction_id = begin_narset_cast(&mut engine, narset);
    let (current_transaction, revision) = current_spell_payment(&mut engine);
    assert_eq!(current_transaction, transaction_id);

    let before_preview = full_snapshot(&engine);
    let proposal = narset_spell_commit(&engine, transaction_id, revision, proposed_mana);
    let preview = engine.preview_payment_scenario(
        CONTROLLER,
        &rv1::PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(proposal.clone()),
            ..Default::default()
        },
    );
    assert!(
        !preview.valid || !preview.complete,
        "{label} Narset payment is not complete: {preview:?}"
    );
    assert_eq!(
        full_snapshot(&engine),
        before_preview,
        "{label} PaymentQuery is read-only"
    );

    let result = engine.apply_scenario_command(
        CONTROLLER,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(proposal)),
        },
    );
    assert!(result.is_err(), "{label} payment cannot commit");
    assert_eq!(
        full_snapshot(&engine),
        before_preview,
        "{label} rejected Commit preserves the pending reservation, stack, and mana"
    );
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .map(|pending| pending.transaction_id),
        Some(transaction_id),
        "Begin remains pending after an incomplete payment"
    );
    assert_eq!(engine.state.objects[&narset].zone, Zone::Stack);
}

fn prepared_narset_on_battlefield(engine: &mut GameEngine) -> u32 {
    // The object is the actual global Narset definition. Direct placement and its printed
    // starting loyalty are prepared state; later ability offers and resolutions are engine-owned.
    let narset = inject_permanent_on_battlefield(engine, 0, NARSET);
    engine
        .state
        .objects
        .get_mut(&narset)
        .expect("prepared actual Narset object")
        .set_counter(CounterKind::Loyalty, 6);
    engine.initial_response_batch();
    assert_eq!(engine.state.objects[&narset].card_id, NARSET);
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        6
    );
    narset
}

fn actual_ability_command(
    engine: &mut GameEngine,
    viewer: i32,
    source_id: u32,
    requested_index: u32,
) -> (u64, u32, bool, RuledCommand) {
    let batch = engine.initial_response_batch();
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("current engine response publishes a battlefield view");
    let player_view = view
        .per_player
        .iter()
        .find(|player| player.player_id == viewer)
        .expect("requested actual player ID receives its own battlefield view");
    let source = player_view
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == source_id)
        .expect("the current view includes the actual Narset source");
    let ability = source
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_index == requested_index)
        .expect("the current actual Narset view includes this ability index");
    let generation = source.zone_change_generation;
    let ability_index = ability.ability_index;
    let activatable = ability.activatable;
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
            rv1::ActivateAbility {
                source_object_id: source.object_id,
                expected_zone_change_generation: generation,
                ability_index,
                source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                ..Default::default()
            },
        )),
    };
    (generation, ability_index, activatable, command)
}

fn reject_actual_ability_atomically(
    engine: &mut GameEngine,
    actor: i32,
    source_id: u32,
    requested_index: u32,
    expected_loyalty: u32,
    expected_stack_len: usize,
    label: &str,
) {
    let (_, _, _, command) = actual_ability_command(engine, CONTROLLER, source_id, requested_index);
    let before = full_snapshot(engine);
    let result = engine.apply_scenario_command(actor, &command);
    assert!(
        result.is_err(),
        "{label} actual Narset activation is illegal"
    );
    assert_eq!(
        full_snapshot(engine),
        before,
        "{label} rejected activation preserves complete state"
    );
    assert_eq!(
        engine.state.objects[&source_id].counter_count(CounterKind::Loyalty),
        expected_loyalty,
        "{label} does not pay loyalty"
    );
    assert_eq!(
        engine.state.stack.len(),
        expected_stack_len,
        "{label} preserves the stack"
    );
}

fn assert_wrong_actor_cannot_use_actual_narset() {
    let mut engine = game(26_110_211);
    let narset = prepared_narset_on_battlefield(&mut engine);
    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    assert_eq!(engine.state.priority_player_id(), OPPONENT);
    reject_actual_ability_atomically(&mut engine, OPPONENT, narset, 0, 6, 0, "wrong controller");
}

fn assert_offturn_actual_narset_cannot_activate() {
    let mut engine = game(26_110_212);
    let narset = prepared_narset_on_battlefield(&mut engine);
    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("opponent passes priority to Narset's controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    reject_actual_ability_atomically(
        &mut engine,
        CONTROLLER,
        narset,
        0,
        6,
        0,
        "controller on the opponent's turn",
    );
}

fn assert_nonmain_actual_narset_cannot_activate() {
    let mut engine = game_at_upkeep(26_110_213);
    let narset = prepared_narset_on_battlefield(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), CONTROLLER);
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    reject_actual_ability_atomically(
        &mut engine,
        CONTROLLER,
        narset,
        0,
        6,
        0,
        "controller outside a main phase",
    );
}

fn assert_stack_blocks_actual_narset() {
    let mut engine = game(26_110_214);
    let narset = prepared_narset_on_battlefield(&mut engine);
    let opt = inject_card_into_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        CONTROLLER,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let opt_slot = hand_index_for_card(&engine, 0, "opt");
    engine
        .apply_scenario_command(CONTROLLER, &cast_spell(opt_slot, vec![]))
        .expect("cast the setup instant to occupy the stack");
    assert_eq!(engine.state.objects[&opt].zone, Zone::Stack);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    reject_actual_ability_atomically(&mut engine, CONTROLLER, narset, 0, 6, 1, "occupied stack");
}

fn assert_insufficient_actual_ultimate_cannot_activate() {
    let mut engine = game(26_110_215);
    let narset = prepared_narset_on_battlefield(&mut engine);
    reject_actual_ability_atomically(
        &mut engine,
        CONTROLLER,
        narset,
        2,
        6,
        0,
        "six loyalty cannot pay Narset's minus nine",
    );
}

fn library_identity_snapshot(engine: &GameEngine, player_id: i32) -> Vec<(u32, u64)> {
    let player_index = engine
        .state
        .player_idx(player_id)
        .expect("known scenario player ID");
    engine.state.players[player_index]
        .library
        .iter()
        .map(|object_id| {
            (
                *object_id,
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn hand_identity_snapshot(engine: &GameEngine, player_id: i32) -> std::collections::BTreeSet<u32> {
    let player_index = engine
        .state
        .player_idx(player_id)
        .expect("known scenario player ID");
    engine.state.players[player_index]
        .hand
        .iter()
        .copied()
        .collect()
}

fn place_on_library_top(engine: &mut GameEngine, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, 0, card_id);
    engine.state.players[0]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[0].library.push_front(object_id);
    object_id
}

fn activate_actual_narset_plus_one(engine: &mut GameEngine, narset: u32) {
    let (_, _, activatable, command) = actual_ability_command(engine, CONTROLLER, narset, 0);
    assert!(
        activatable,
        "the current main-phase offer permits Narset's +1"
    );
    engine
        .apply_scenario_command(CONTROLLER, &command)
        .expect("activate the actual registered Narset +1");
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        7,
        "the loyalty cost is paid when the ability is activated"
    );
}

fn assert_no_reveal(batch: &RuledEventBatch) {
    assert!(
        batch
            .events
            .iter()
            .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))),
        "this private look path does not publicly reveal a card"
    );
}

fn assert_no_public_card_name(batch: &RuledEventBatch, name: &str) {
    for event in &batch.events {
        if let Some(Ev::Log(log)) = &event.ev {
            if log.visible_to_player_id.is_none() {
                assert!(
                    !log.text.contains(name),
                    "public log exposed private library identity {name}: {}",
                    log.text
                );
            }
        }
    }
}

fn choice_command(prompt_id: u64, chosen_object_ids: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: prompt_id,
                chosen_object_ids,
                ..Default::default()
            },
        )),
    }
}

fn assert_current_look_parent(
    engine: &GameEngine,
    narset: u32,
    choice: &rv1::ResolutionChoiceRequired,
) -> u64 {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Narset's resolving ability is parked at the current look choice");
    let prompt_id = pending.resolution_prompt_id.get();
    assert_ne!(prompt_id, 0);
    assert_eq!(choice.resolution_prompt_id, prompt_id);
    assert_eq!(pending.deciding_player, CONTROLLER);
    assert_eq!(choice.deciding_player_id, CONTROLLER);
    assert_eq!(
        pending.presentation.source_object_id,
        choice.source_object_id
    );
    let ResolutionContinuation::LibraryLook {
        stack,
        stage: PendingLibraryLookStage::ChooseToHand { .. },
        ..
    } = &pending.continuation
    else {
        panic!("the actual Narset +1 awaits its library-to-hand decision")
    };
    assert_eq!(stack.item.card_id, NARSET);
    assert_eq!(stack.item.controller, CONTROLLER);
    assert_eq!(stack.item.id, choice.source_object_id);
    assert_eq!(stack.item.source_permanent_id, Some(narset));
    assert_eq!(
        stack.item.source_zone_change,
        engine
            .state
            .zone_change_generation
            .get(&narset)
            .copied()
            .unwrap_or_default()
    );
    assert!(engine.state.stack.is_empty());
    prompt_id
}

fn assert_library_look_choice(
    engine: &GameEngine,
    batch: &RuledEventBatch,
    narset: u32,
    top: u32,
    selectable: bool,
    bounds: (u32, u32),
    card_name: &str,
) -> (rv1::ResolutionChoiceRequired, u64) {
    let choice = find_resolution_choice(batch)
        .expect("a nonempty library publishes the private top-card choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((choice.min, choice.max), bounds);
    assert_eq!(choice.candidate_object_ids, vec![top]);
    assert_eq!(choice.candidate_selectable, vec![selectable]);
    assert!(choice.public_reveal.is_none());
    assert_current_look_parent(engine, narset, &choice);
    assert_no_reveal(batch);
    assert_no_public_card_name(batch, card_name);
    (choice.clone(), choice.resolution_prompt_id)
}

fn assert_wrong_player_and_stale_choice_are_atomic(
    engine: &mut GameEngine,
    prompt_id: u64,
    candidate: u32,
) {
    for (actor, submitted_prompt_id, label, stale) in [
        (OPPONENT, prompt_id, "wrong player", false),
        (
            CONTROLLER,
            prompt_id.saturating_add(1),
            "stale prompt",
            true,
        ),
    ] {
        if stale {
            assert_ne!(submitted_prompt_id, prompt_id);
        }
        let before = full_snapshot(engine);
        let result =
            engine.apply_command(actor, &choice_command(submitted_prompt_id, vec![candidate]));
        assert!(result.is_err(), "{label} choice must be rejected");
        assert_eq!(
            full_snapshot(engine),
            before,
            "{label} choice preserves the pending parent and all card identities"
        );
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .expect("rejected choice retains its parent")
                .resolution_prompt_id
                .get(),
            prompt_id
        );
    }
}

fn assert_eligible_accept_case() {
    let mut engine = game(26_110_221);
    let narset = prepared_narset_on_battlefield(&mut engine);
    let top = place_on_library_top(&mut engine, "opt");
    let library_before = library_identity_snapshot(&engine, CONTROLLER);
    let hand_before = hand_identity_snapshot(&engine, CONTROLLER);
    let draws_before = engine
        .state
        .turn_history
        .current
        .player(CONTROLLER)
        .cards_drawn;
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&narset)
        .copied()
        .unwrap_or_default();

    activate_actual_narset_plus_one(&mut engine, narset);
    let choice_batch = pass_both_players_with_batch(&mut engine);
    let (_, prompt_id) =
        assert_library_look_choice(&engine, &choice_batch, narset, top, true, (0, 1), "Opt");
    assert_eq!(
        library_identity_snapshot(&engine, CONTROLLER),
        library_before
    );
    assert_eq!(hand_identity_snapshot(&engine, CONTROLLER), hand_before);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(CONTROLLER)
            .cards_drawn,
        draws_before
    );
    assert_wrong_player_and_stale_choice_are_atomic(&mut engine, prompt_id, top);

    let accepted = engine
        .apply_command(CONTROLLER, &choice_command(prompt_id, vec![top]))
        .expect("accept the exact current eligible top card");
    let reveals: Vec<_> = accepted
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .collect();
    assert_eq!(reveals.len(), 1, "acceptance reveals one selected card");
    assert_eq!(reveals[0].cards.len(), 1);
    assert_eq!(reveals[0].cards[0].object_id, top);
    assert_eq!(reveals[0].cards[0].card_name, "Opt");
    assert!(!reveals[0].reveal_id.is_empty());
    assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation.get(&top).copied(),
        Some(library_before[0].1 + 1)
    );
    assert_eq!(
        library_identity_snapshot(&engine, CONTROLLER),
        library_before[1..].to_vec(),
        "only the accepted top ObjectId leaves its exact library position"
    );
    let mut expected_hand = hand_before;
    assert!(expected_hand.insert(top));
    assert_eq!(hand_identity_snapshot(&engine, CONTROLLER), expected_hand);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(CONTROLLER)
            .cards_drawn,
        draws_before,
        "moving the revealed card to hand is not a draw"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&narset].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&narset)
            .copied()
            .unwrap_or_default(),
        source_generation
    );
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        7
    );
}

fn assert_eligible_decline_case() {
    let mut engine = game(26_110_222);
    let narset = prepared_narset_on_battlefield(&mut engine);
    let top = place_on_library_top(&mut engine, "opt");
    let library_before = library_identity_snapshot(&engine, CONTROLLER);
    let hand_before = hand_identity_snapshot(&engine, CONTROLLER);
    let draws_before = engine
        .state
        .turn_history
        .current
        .player(CONTROLLER)
        .cards_drawn;

    activate_actual_narset_plus_one(&mut engine, narset);
    let choice_batch = pass_both_players_with_batch(&mut engine);
    let (_, prompt_id) =
        assert_library_look_choice(&engine, &choice_batch, narset, top, true, (0, 1), "Opt");
    let declined = engine
        .apply_command(CONTROLLER, &choice_command(prompt_id, vec![]))
        .expect("decline the eligible private top card");
    assert_no_reveal(&declined);
    assert_no_public_card_name(&declined, "Opt");
    assert_eq!(
        library_identity_snapshot(&engine, CONTROLLER),
        library_before
    );
    assert_eq!(hand_identity_snapshot(&engine, CONTROLLER), hand_before);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(CONTROLLER)
            .cards_drawn,
        draws_before
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        7
    );
}

fn assert_nonmatching_top_case(seed: u64, card_id: &str, card_name: &str) {
    let mut engine = game(seed);
    let narset = prepared_narset_on_battlefield(&mut engine);
    let top = place_on_library_top(&mut engine, card_id);
    let library_before = library_identity_snapshot(&engine, CONTROLLER);
    let hand_before = hand_identity_snapshot(&engine, CONTROLLER);
    let draws_before = engine
        .state
        .turn_history
        .current
        .player(CONTROLLER)
        .cards_drawn;

    activate_actual_narset_plus_one(&mut engine, narset);
    let choice_batch = pass_both_players_with_batch(&mut engine);
    let (choice, prompt_id) = assert_library_look_choice(
        &engine,
        &choice_batch,
        narset,
        top,
        false,
        (0, 0),
        card_name,
    );
    assert_eq!(choice.candidate_object_ids, vec![top]);
    let declined = engine
        .apply_command(CONTROLLER, &choice_command(prompt_id, vec![]))
        .expect("complete the nonselectable zero-card private look");
    assert_no_reveal(&declined);
    assert_no_public_card_name(&declined, card_name);
    assert_eq!(
        library_identity_snapshot(&engine, CONTROLLER),
        library_before
    );
    assert_eq!(hand_identity_snapshot(&engine, CONTROLLER), hand_before);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(CONTROLLER)
            .cards_drawn,
        draws_before
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        7
    );
}

fn assert_empty_library_case() {
    let mut engine = game(26_110_225);
    let narset = prepared_narset_on_battlefield(&mut engine);
    let emptied: Vec<_> = engine.state.players[0].library.drain(..).collect();
    for object_id in emptied {
        engine.state.objects.remove(&object_id);
        engine.state.zone_change_generation.remove(&object_id);
    }
    assert!(engine.state.players[0].library.is_empty());
    let draws_before = engine
        .state
        .turn_history
        .current
        .player(CONTROLLER)
        .cards_drawn;

    activate_actual_narset_plus_one(&mut engine, narset);
    let completed = pass_both_players_with_batch(&mut engine);
    assert!(
        find_resolution_choice(&completed).is_none(),
        "the inspected Phase A flow completes an empty library without a prompt"
    );
    assert_no_reveal(&completed);
    assert!(engine.state.players[0].library.is_empty());
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(CONTROLLER)
            .cards_drawn,
        draws_before
    );
    assert!(engine.state.players.iter().all(|player| !player.has_lost));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        7
    );
}

fn assert_narset_cast_and_first_plus_one() {
    let mut engine = game(26_110_203);
    let narset = inject_card_into_hand(&mut engine, 0, NARSET);
    give_mana(
        &mut engine,
        CONTROLLER,
        ManaGift {
            w: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let transaction_id = begin_narset_cast(&mut engine, narset);
    pay_narset_cast(
        &mut engine,
        transaction_id,
        rv1::PaymentMana {
            w: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        (
            engine.state.players[0].mana_pool.white,
            engine.state.players[0].mana_pool.blue,
            engine.state.players[0].mana_pool.colorless,
        ),
        (0, 0, 0)
    );
    assert_eq!(engine.state.objects[&narset].zone, Zone::Stack);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&narset].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        6,
        "actual registered Narset enters with printed loyalty six"
    );

    let top = place_on_library_top(&mut engine, "opt");
    let library_before = library_identity_snapshot(&engine, CONTROLLER);
    let hand_before = hand_identity_snapshot(&engine, CONTROLLER);
    let draws_before = engine
        .state
        .turn_history
        .current
        .player(CONTROLLER)
        .cards_drawn;
    let starting_turn = engine.state.turn_instance;
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&narset)
        .copied()
        .unwrap_or_default();
    let (_, ability_index, activatable, command) =
        actual_ability_command(&mut engine, CONTROLLER, narset, 0);
    assert_eq!(ability_index, 0);
    assert!(activatable, "the current Narset +1 offer is activatable");
    engine
        .apply_scenario_command(CONTROLLER, &command)
        .expect("activate actual Narset +1 from the current offer");
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        7
    );
    assert_eq!(engine.state.turn_instance, starting_turn);
    assert_eq!(engine.state.stack.len(), 1);

    let choice_batch = pass_both_players_with_batch(&mut engine);
    let (_, prompt_id) =
        assert_library_look_choice(&engine, &choice_batch, narset, top, true, (0, 1), "Opt");
    assert_eq!(
        library_identity_snapshot(&engine, CONTROLLER),
        library_before
    );
    assert_eq!(hand_identity_snapshot(&engine, CONTROLLER), hand_before);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(CONTROLLER)
            .cards_drawn,
        draws_before
    );
    let accepted = engine
        .apply_command(CONTROLLER, &choice_command(prompt_id, vec![top]))
        .expect("finish the current private choice and its Narset parent");
    let reveals: Vec<_> = accepted
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .collect();
    assert_eq!(reveals.len(), 1);
    assert_eq!(reveals[0].cards.len(), 1);
    assert_eq!(reveals[0].cards[0].object_id, top);
    assert_eq!(reveals[0].cards[0].card_name, "Opt");
    assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
    assert_eq!(
        library_identity_snapshot(&engine, CONTROLLER),
        library_before[1..].to_vec()
    );
    let mut expected_hand = hand_before;
    assert!(expected_hand.insert(top));
    assert_eq!(hand_identity_snapshot(&engine, CONTROLLER), expected_hand);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(CONTROLLER)
            .cards_drawn,
        draws_before
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&narset].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&narset)
            .copied()
            .unwrap_or_default(),
        source_generation
    );
    assert_eq!(engine.state.turn_instance, starting_turn);

    reject_actual_ability_atomically(
        &mut engine,
        CONTROLLER,
        narset,
        ability_index,
        7,
        0,
        "same-turn second loyalty activation after the first parent finishes",
    );
}

#[test]
fn narset_cast_and_loyalty_timing_pay_printed_costs() {
    reject_narset_cast_payment(
        26_110_201,
        ManaGift {
            w: 1,
            u: 1,
            c: 1,
            ..Default::default()
        },
        rv1::PaymentMana {
            w: 1,
            u: 1,
            c: 1,
            ..Default::default()
        },
        "insufficient",
    );
    reject_narset_cast_payment(
        26_110_202,
        ManaGift {
            r: 4,
            ..Default::default()
        },
        rv1::PaymentMana {
            r: 4,
            ..Default::default()
        },
        "wrong-color",
    );
    assert_narset_cast_and_first_plus_one();
    assert_wrong_actor_cannot_use_actual_narset();
    assert_offturn_actual_narset_cannot_activate();
    assert_nonmain_actual_narset_cannot_activate();
    assert_stack_blocks_actual_narset();
    assert_insufficient_actual_ultimate_cannot_activate();
}

#[test]
fn narset_plus_one_preserves_private_top_or_reveals_only_the_accepted_card() {
    assert_eligible_accept_case();
    assert_eligible_decline_case();
    assert_nonmatching_top_case(26_110_223, "grizzly_bears", "Grizzly Bears");
    assert_nonmatching_top_case(26_110_224, "island", "Island");
    assert_empty_library_case();
}

fn narset_serialized_state_field(engine: &GameEngine, field: &str) -> serde_json::Value {
    full_snapshot(engine)["state"][field].clone()
}

fn narset_active_event_observer_count(engine: &GameEngine) -> usize {
    narset_serialized_state_field(engine, "active_event_observers")
        .as_array()
        .expect("diagnostics expose active event observers")
        .len()
}

fn narset_rebound_grants(engine: &GameEngine) -> serde_json::Value {
    narset_serialized_state_field(engine, "rebound_spell_grants")
}

fn narset_scheduled_rebounds(engine: &GameEngine) -> serde_json::Value {
    narset_serialized_state_field(engine, "scheduled_rebounds")
}

fn narset_assert_no_rebound_schedule_or_grant(engine: &GameEngine, context: &str) {
    assert_eq!(
        narset_rebound_grants(engine),
        serde_json::json!([]),
        "{context} has no captured Rebound grant"
    );
    assert_eq!(
        narset_scheduled_rebounds(engine),
        serde_json::json!([]),
        "{context} has no scheduled Rebound"
    );
}

fn narset_capture_priority_round(engine: &mut GameEngine) -> rv1::RuledEventBatch {
    let live_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let passes_needed = live_players
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("the fixture has a consistent live-player priority pass count");
    let mut last_batch = None;
    for _ in 0..passes_needed {
        let actor = engine.state.priority_player_id();
        last_batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("the current priority holder passes to complete the round"),
        );
    }
    last_batch.expect("a live player still has priority to pass")
}

fn narset_activate_minus_two(engine: &mut GameEngine, narset: u32) -> u64 {
    let controller = engine.state.objects[&narset].controller;
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.active_player_id(), controller);
    assert_eq!(engine.state.priority_player_id(), controller);
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        6
    );
    let (source_generation, ability_index, activatable, activation) =
        actual_ability_command(engine, controller, narset, 1);
    assert_eq!(ability_index, 1);
    assert!(activatable, "the current actual Narset -2 is offered");
    engine
        .apply_scenario_command(controller, &activation)
        .expect("activate actual Narset -2 from its current receipt");
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        4,
        "the actual -2 costs two loyalty"
    );
    assert_eq!(engine.state.objects[&narset].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&narset)
            .copied()
            .unwrap_or(0),
        source_generation,
        "activating -2 preserves Narset's physical incarnation"
    );
    assert_eq!(engine.state.stack.len(), 1);
    narset_capture_priority_round(engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        narset_active_event_observer_count(engine),
        1,
        "resolving the actual -2 arms one next qualifying hand spell observer"
    );
    narset_assert_no_rebound_schedule_or_grant(engine, "after -2 resolves");
    source_generation
}

fn narset_cast_actual_hand_spell(
    engine: &mut GameEngine,
    player_seat: usize,
    card_id: &str,
    mana: ManaGift,
) -> (
    u32,
    tricerules_core::state::StackObjectRef,
    rv1::RuledEventBatch,
) {
    let player_id = engine.state.players[player_seat].id;
    ensure_in_hand(engine, player_seat, card_id);
    give_mana(engine, player_id, mana);
    let hand_index = hand_index_for_card(engine, player_seat, card_id);
    let object_id = engine.state.players[player_seat].hand[hand_index];
    let old_stack_ids = engine
        .state
        .stack
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let mut command = cast_spell(hand_index, Vec::new());
    authoring_actions::pay(engine, player_id, &mut command)
        .expect("pay the actual hand spell from its current offer");
    let mana_pool_before_commit = engine.state.players[player_seat].mana_pool;
    let batch = engine
        .apply_scenario_command(player_id, &command)
        .expect("commit the actual hand spell");
    assert_eq!(
        engine.state.players[player_seat].mana_pool,
        tricerules_core::state::ManaPool::default(),
        "the exact prepared mana pays the actual spell"
    );
    let spell = engine
        .state
        .stack
        .iter()
        .find(|item| {
            !old_stack_ids.contains(&item.id)
                && item.card_id == card_id
                && item.cast_by == Some(player_id)
                && !item.is_triggered
                && !item.is_copy
        })
        .expect("the actual hand card is the newly committed physical spell");
    assert_eq!(
        spell.cast_method,
        tricerules_core::state::SpellCastMethod::Normal
    );
    let occurrence = spell
        .cast_occurrence
        .expect("the actual hand spell has its physical cast occurrence");
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(Ev::StackPushed(pushed)) if pushed.object_id == occurrence.object_id
                )
            })
            .count(),
        1,
        "the physical hand spell publishes exactly one StackPushed"
    );
    assert_ne!(
        mana_pool_before_commit,
        tricerules_core::state::ManaPool::default(),
        "the actual spell was paid from the prepared mana pool"
    );
    (object_id, occurrence, batch)
}

fn narset_assert_grant_trigger_for_occurrence(
    engine: &GameEngine,
    narset: u32,
    narset_generation: u64,
    occurrence: tricerules_core::state::StackObjectRef,
    cast_batch: &rv1::RuledEventBatch,
) {
    let spell_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.cast_occurrence == Some(occurrence))
        .expect("the exact hand cast remains on the stack");
    let triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect::<Vec<_>>();
    assert_eq!(
        triggers.len(),
        1,
        "one ordinary Rebound grant trigger is staged"
    );
    let trigger = triggers[0];
    let trigger_index = engine
        .state
        .stack
        .iter()
        .position(|item| item.id == trigger.id)
        .expect("the ordinary grant trigger remains on the stack");
    assert!(
        trigger_index > spell_index,
        "the ordinary grant trigger is above the actual hand spell"
    );
    assert!(!trigger.is_copy);
    assert_eq!(trigger.controller, engine.state.objects[&narset].owner);
    assert_eq!(trigger.source_permanent_id, Some(narset));
    assert_eq!(trigger.source_zone_change, narset_generation);
    let serialized_trigger =
        serde_json::to_value(trigger).expect("serialize the staged Narset grant trigger");
    assert_eq!(
        serialized_trigger["trigger_context"]["triggering_spell"],
        serde_json::to_value(Some(occurrence))
            .expect("serialize the exact committed Narset spell occurrence"),
        "the Narset grant trigger captures this exact actual hand cast"
    );
    assert!(
        trigger.triggered_ability.is_some(),
        "the Rebound grant is an ordinary triggered ability"
    );
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        2,
        "the actual Divination and its ordinary grant trigger are both pushed"
    );
    assert_eq!(
        narset_active_event_observer_count(engine),
        0,
        "the one-shot hand spell observer is consumed by this cast"
    );
    narset_assert_no_rebound_schedule_or_grant(engine, "before the grant trigger resolves");
}

fn narset_expected_rebound_grant(
    occurrence: tricerules_core::state::StackObjectRef,
) -> serde_json::Value {
    serde_json::json!([[
        occurrence.object_id,
        occurrence
            .zone_change_generation
            .map(|generation| generation.to_string())
    ]])
}

fn narset_resolve_original_rebound_divination(
    engine: &mut GameEngine,
    controller: i32,
    occurrence: tricerules_core::state::StackObjectRef,
) -> (u64, u64, serde_json::Value) {
    let hand_before_resolution = engine.state.players[0].hand.len();
    let resolution = narset_capture_priority_round(engine);
    let resolved = resolution
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::StackResolved(resolved)) => Some(resolved),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(resolved.len(), 1, "the actual Divination resolves once");
    assert_eq!(resolved[0].object_id, occurrence.object_id);
    assert_eq!(
        resolved[0].destination,
        rv1::StackResolveDestination::Exile as i32,
        "the granted Rebound exiles the spell that resolved"
    );
    assert_eq!(resolved[0].owner_player_id, Some(controller));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_resolution + 2
    );
    assert_eq!(
        engine.state.objects[&occurrence.object_id].zone,
        Zone::Exile
    );
    assert_eq!(
        engine.state.objects[&occurrence.object_id].owner,
        controller
    );

    let stack_generation = occurrence
        .zone_change_generation
        .expect("the original Divination has its exact stack generation");
    let exile_generation = engine
        .state
        .zone_change_generation
        .get(&occurrence.object_id)
        .copied()
        .expect("the actual Divination has its current Exile generation");
    assert_eq!(exile_generation, stack_generation + 1);
    assert_eq!(narset_rebound_grants(engine), serde_json::json!([]));
    let saved_schedule = narset_scheduled_rebounds(engine);
    let records = saved_schedule
        .as_array()
        .expect("diagnostics expose delayed Rebound schedules");
    assert_eq!(
        records.len(),
        1,
        "one exact Divination Rebound is scheduled"
    );
    let record = &records[0];
    assert_eq!(
        record["source"]["object_id"],
        serde_json::json!(occurrence.object_id)
    );
    assert_eq!(
        record["source"]["zone_change_generation"],
        serde_json::json!(stack_generation.to_string())
    );
    assert_eq!(
        record["exiled"]["object_id"],
        serde_json::json!(occurrence.object_id)
    );
    assert_eq!(
        record["exiled"]["zone_change_generation"],
        serde_json::json!(exile_generation.to_string())
    );
    assert_eq!(record["controller"], serde_json::json!(controller));
    assert_eq!(record["card_id"], serde_json::json!("divination"));
    assert_eq!(record["card_name"], serde_json::json!("Divination"));
    assert_eq!(record["source_face_index"], serde_json::json!("0"));
    (stack_generation, exile_generation, saved_schedule)
}

fn narset_advance_to_next_rebound_upkeep(
    engine: &mut GameEngine,
    controller: i32,
    opponent: i32,
    object_id: u32,
    stack_generation: u64,
    exile_generation: u64,
    saved_schedule: &serde_json::Value,
) -> u32 {
    end_active_turn(engine, controller);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), opponent);
    assert!(engine.state.stack.is_empty());
    assert_eq!(narset_scheduled_rebounds(engine), *saved_schedule);
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != object_id));
    let opponent_upkeep = engine.initial_response_batch();
    assert!(opponent_upkeep.legal_by_player[&controller]
        .zone_cast_actions
        .iter()
        .all(|action| action.object_id != object_id));
    assert!(
        engine.state.stack.iter().all(|item| !item.is_triggered),
        "the opponent's upkeep creates no Rebound trigger"
    );

    pass_priority_round(engine);
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    end_active_turn(engine, opponent);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), controller);
    assert_eq!(engine.state.stack.len(), 1);
    let trigger = &engine.state.stack[0];
    assert!(trigger.is_triggered);
    assert!(!trigger.is_copy);
    assert_eq!(trigger.card_id, "divination");
    assert_eq!(trigger.controller, controller);
    assert_eq!(trigger.source_permanent_id, Some(object_id));
    assert_eq!(trigger.source_zone_change, stack_generation);
    assert_eq!(
        trigger.trigger_context.observed_object,
        Some(tricerules_core::state::TriggerObjectRef {
            object_id,
            zone_change_generation: exile_generation,
            controller_at_event: controller,
        })
    );
    assert!(narset_scheduled_rebounds(engine)
        .as_array()
        .is_some_and(Vec::is_empty));
    trigger.id
}

fn narset_accept_current_rebound_offer(
    engine: &mut GameEngine,
    controller: i32,
    object_id: u32,
    exile_generation: u64,
    trigger_id: u32,
) -> tricerules_core::state::StackObjectRef {
    let trigger_resolution = narset_capture_priority_round(engine);
    assert!(engine.state.stack.is_empty());
    let special_offer = find_resolution_choice(&trigger_resolution)
        .expect("the actual Rebound trigger publishes its current optional cast offer");
    assert_eq!(
        special_offer.choice_kind,
        rv1::ChoiceKind::SpecialCast as i32
    );
    assert_eq!(special_offer.source_object_id, trigger_id);
    assert_eq!(special_offer.candidate_object_ids, vec![object_id]);
    assert_eq!(special_offer.candidate_selectable, [true]);

    let (deciding_player, prompt_id) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual trigger parks at its current SpecialCast choice");
        assert_eq!(pending.deciding_player, controller);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
        assert_eq!(pending.presentation.candidates, vec![object_id]);
        let tricerules_core::state::ResolutionContinuation::SpecialCast {
            stack,
            exiled,
            face_index,
            method,
            cost,
            permission_scope,
            completion,
            ..
        } = &pending.continuation
        else {
            panic!("the actual Rebound trigger owns the SpecialCast continuation");
        };
        assert_eq!(stack.item.id, trigger_id);
        assert_eq!(exiled.object_id, object_id);
        assert_eq!(exiled.zone_change_generation, exile_generation);
        assert_eq!(*face_index, 0);
        assert_eq!(*method, tricerules_core::state::SpellCastMethod::Permission);
        assert_eq!(*cost, tricerules_cards::mana::ManaCost::default());
        assert_eq!(
            *permission_scope,
            Some(tricerules_core::state::ExilePlayPermissionScope::CastCard)
        );
        assert!(matches!(
            completion,
            tricerules_core::state::SpecialCastCompletion::ResumeStack
        ));
        (pending.deciding_player, pending.resolution_prompt_id.get())
    };
    assert_ne!(prompt_id, 0);
    assert_eq!(special_offer.resolution_prompt_id, prompt_id);

    let actions = engine.initial_response_batch().legal_by_player[&controller]
        .zone_cast_actions
        .iter()
        .filter(|action| action.object_id == object_id)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 1, "one current permission is offered");
    let action = &actions[0];
    assert_eq!(action.source_zone, rv1::CastSourceZone::Exile as i32);
    assert_eq!(action.zone_change_generation, exile_generation);
    assert_eq!(action.face_index, 0);
    assert_eq!(action.cast_method, rv1::CastMethod::Permission as i32);
    assert_eq!(
        action.cost, "",
        "the published free-cast cost is the empty string"
    );
    let permission_id = action
        .casting_permission_id
        .expect("the exact current Rebound offer has a permission ID");
    assert_ne!(permission_id, 0);

    let begin = engine
        .apply_scenario_command(
            deciding_player,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: prompt_id,
                        decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                        spell_cast_announcement: Some(rv1::SpellCastAnnouncement {
                            source: Some(exile_cast_source(object_id, exile_generation)),
                            face_index: action.face_index,
                            cast_method: action.cast_method,
                            casting_permission_id: Some(permission_id),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("accept the current exact Divination Rebound offer");
    assert_eq!(
        begin
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(Ev::StackPushed(pushed)) if pushed.object_id == object_id
                )
            })
            .count(),
        0,
        "the pending cast does not publish StackPushed before payment commit"
    );
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Stack);
    assert_eq!(
        engine.state.zone_change_generation[&object_id],
        exile_generation + 1
    );
    let pending_cast = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the accepted Rebound offer opens its staged spell payment");
    assert!(pending_cast.resolution_time_offer);
    assert_eq!(pending_cast.caster, controller);
    assert_eq!(pending_cast.reserved_object_id, object_id);
    assert_eq!(pending_cast.original_zone, Zone::Exile);
    assert_eq!(
        pending_cast.original_zone_change_generation,
        exile_generation
    );
    assert_eq!(
        pending_cast.announcement.casting_permission_id,
        Some(permission_id)
    );

    let mana_before_preview = engine.state.players[engine
        .state
        .player_idx(controller)
        .expect("the Rebound controller remains in the game")]
    .mana_pool;
    let transaction_id = pending_cast.transaction_id;
    let revision = engine.state.command_index;
    let preview = engine.preview_payment_scenario(
        controller,
        &rv1::PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(rv1::CommitSpellCast {
                transaction_id,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(
        preview.valid && preview.complete,
        "free Rebound preview: {preview:?}"
    );
    assert_eq!(
        preview.total_cost, "{0}",
        "the free Rebound preview uses the zero-mana cost representation"
    );
    assert_eq!(engine.state.command_index, revision, "preview is read-only");
    let selection = preview
        .selection
        .clone()
        .expect("the complete zero-cost preview returns a payment selection");
    assert_eq!(selection.expected_state_revision, revision);
    let committed = engine
        .apply_scenario_command(
            controller,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id,
                        payment: preview.selection,
                        restricted_mana: preview.restricted_mana,
                    },
                )),
            },
        )
        .expect("commit the current free physical Divination cast from Exile");
    let controller_index = engine
        .state
        .player_idx(controller)
        .expect("the controller remains in the game");
    assert_eq!(
        engine.state.players[controller_index].mana_pool, mana_before_preview,
        "Rebound waives the mana cost"
    );
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.stack.iter().any(|item| item.id == trigger_id));
    assert!(committed.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger_id
        )
    }));
    assert_eq!(
        committed
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(Ev::StackPushed(pushed)) if pushed.object_id == object_id
                )
            })
            .count(),
        1,
        "payment commit publishes the actual physical spell once"
    );
    let recast = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == object_id)
        .expect("the exact exiled Divination is now the physical spell");
    assert_eq!(recast.card_id, "divination");
    assert_eq!(recast.controller, controller);
    assert_eq!(recast.cast_by, Some(controller));
    assert_eq!(recast.face_index, 0);
    assert_eq!(
        recast.cast_method,
        tricerules_core::state::SpellCastMethod::Permission
    );
    assert!(!recast.is_copy);
    assert!(!recast.is_triggered);
    let occurrence = recast
        .cast_occurrence
        .expect("the Rebound cast has its own exact occurrence");
    assert_eq!(
        occurrence.zone_change_generation,
        Some(exile_generation + 1)
    );
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != object_id));
    narset_assert_no_rebound_schedule_or_grant(engine, "after the Rebound cast");
    occurrence
}

fn narset_resolve_rebounded_divination(
    engine: &mut GameEngine,
    controller: i32,
    object_id: u32,
    exile_generation: u64,
    original_occurrence: tricerules_core::state::StackObjectRef,
    rebound_occurrence: tricerules_core::state::StackObjectRef,
) {
    assert_ne!(
        rebound_occurrence, original_occurrence,
        "the permission cast is a new physical cast occurrence"
    );
    let controller_index = engine
        .state
        .player_idx(controller)
        .expect("the controller remains in the game");
    let hand_before_resolution = engine.state.players[controller_index].hand.len();
    let resolution = narset_capture_priority_round(engine);
    let resolved = resolution
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::StackResolved(resolved)) => Some(resolved),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        resolved.len(),
        1,
        "the actual free Divination resolves once"
    );
    assert_eq!(resolved[0].object_id, object_id);
    assert_eq!(
        resolved[0].destination,
        rv1::StackResolveDestination::Graveyard as i32,
        "a spell cast from Exile goes to its owner's graveyard"
    );
    assert_eq!(resolved[0].owner_player_id, Some(controller));
    assert_eq!(
        engine.state.players[controller_index].hand.len(),
        hand_before_resolution + 2
    );
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&object_id],
        exile_generation + 2
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != object_id));
    assert!(engine.state.stack.iter().all(|item| !item.is_triggered));
    narset_assert_no_rebound_schedule_or_grant(engine, "after Rebound resolves");
}

fn narset_assert_no_later_rebound_upkeep(
    engine: &mut GameEngine,
    controller: i32,
    opponent: i32,
    object_id: u32,
) {
    pass_priority_round(engine);
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    end_active_turn(engine, controller);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), opponent);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.stack.iter().all(|item| !item.is_triggered));
    narset_assert_no_rebound_schedule_or_grant(engine, "the following opponent upkeep");
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != object_id));

    pass_priority_round(engine);
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    end_active_turn(engine, opponent);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), controller);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(
        narset_active_event_observer_count(engine) == 0,
        "the consumed observer and grant do not create a Rebound loop"
    );
    narset_assert_no_rebound_schedule_or_grant(engine, "the next controller upkeep");
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .all(|permission| permission.object_id != object_id));
}

#[test]
fn narset_minus_two_grants_then_rebounds_the_actual_hand_spell() {
    let mut engine = game(26_110_231);
    let narset = prepared_narset_on_battlefield(&mut engine);
    let divination = inject_card_into_hand(&mut engine, 0, "divination");
    let controller = engine.state.players[0].id;
    let opponent = engine.state.players[1].id;
    let narset_generation = narset_activate_minus_two(&mut engine, narset);
    let (cast_object, original_occurrence, cast_batch) = narset_cast_actual_hand_spell(
        &mut engine,
        0,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(cast_object, divination);
    narset_assert_grant_trigger_for_occurrence(
        &engine,
        narset,
        narset_generation,
        original_occurrence,
        &cast_batch,
    );
    narset_capture_priority_round(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "only the original Divination remains after the grant trigger resolves"
    );
    assert_eq!(
        narset_rebound_grants(&engine),
        narset_expected_rebound_grant(original_occurrence),
        "the exact cast occurrence receives Rebound when the trigger resolves"
    );
    assert_eq!(narset_active_event_observer_count(&engine), 0);
    let (stack_generation, exile_generation, saved_schedule) =
        narset_resolve_original_rebound_divination(&mut engine, controller, original_occurrence);
    let trigger_id = narset_advance_to_next_rebound_upkeep(
        &mut engine,
        controller,
        opponent,
        divination,
        stack_generation,
        exile_generation,
        &saved_schedule,
    );
    let rebound_occurrence = narset_accept_current_rebound_offer(
        &mut engine,
        controller,
        divination,
        exile_generation,
        trigger_id,
    );
    narset_resolve_rebounded_divination(
        &mut engine,
        controller,
        divination,
        exile_generation,
        original_occurrence,
        rebound_occurrence,
    );
    narset_assert_no_later_rebound_upkeep(&mut engine, controller, opponent, divination);
}

#[test]
fn narset_minus_two_unconsumed_effect_expires_at_cleanup() {
    let mut engine = game(26_110_232);
    let narset = prepared_narset_on_battlefield(&mut engine);
    let divination = inject_card_into_hand(&mut engine, 0, "divination");
    let controller = engine.state.players[0].id;
    let opponent = engine.state.players[1].id;
    narset_activate_minus_two(&mut engine, narset);
    assert_eq!(narset_active_event_observer_count(&engine), 1);

    end_active_turn(&mut engine, controller);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), opponent);
    assert_eq!(
        narset_active_event_observer_count(&engine),
        0,
        "the unconsumed this-turn Rebound observer expires at cleanup"
    );
    narset_assert_no_rebound_schedule_or_grant(&engine, "after unconsumed cleanup");
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    end_active_turn(&mut engine, opponent);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.active_player_id(), controller);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), controller);
    assert_eq!(engine.state.objects[&narset].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        4
    );
    assert_eq!(engine.state.objects[&divination].zone, Zone::Hand);
    assert_eq!(narset_active_event_observer_count(&engine), 0);

    let (cast_object, occurrence, cast_batch) = narset_cast_actual_hand_spell(
        &mut engine,
        0,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(cast_object, divination);
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(_))))
            .count(),
        1,
        "the next-turn hand Divination has no surviving Rebound trigger"
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].cast_occurrence, Some(occurrence));
    assert!(!engine.state.stack[0].is_triggered);
    narset_assert_no_rebound_schedule_or_grant(&engine, "the next-turn hand Divination cast");

    let hand_before_resolution = engine.state.players[0].hand.len();
    let resolution = narset_capture_priority_round(&mut engine);
    let resolved = resolution
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackResolved(resolved)) => Some(resolved),
            _ => None,
        })
        .expect("the next-turn Divination resolves");
    assert_eq!(resolved.object_id, divination);
    assert_eq!(
        resolved.destination,
        rv1::StackResolveDestination::Graveyard as i32
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_resolution + 2
    );
    assert_eq!(engine.state.objects[&divination].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
    narset_assert_no_rebound_schedule_or_grant(&engine, "after the ungranted Divination resolves");
}
fn narset_three_player_ultimate_fixture(seed: u64) -> (GameEngine, [i32; 3], u32) {
    let registry = tricerules_cards::registry::global();
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[17, 29, 41],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("prepared three-player actual Narset fixture");
    advance_to_main1_from_game_start(&mut engine);

    let player_ids = [
        engine.state.players[0].id,
        engine.state.players[1].id,
        engine.state.players[2].id,
    ];
    for (seat, player_id) in player_ids.iter().copied().enumerate() {
        assert_ne!(player_id as usize, seat, "seat indices are not PlayerIds");
        assert_eq!(
            player_ids
                .iter()
                .filter(|other| **other == player_id)
                .count(),
            1,
            "the three fixture seats have distinct actual PlayerIds"
        );
    }
    assert_eq!(engine.state.active_player_id(), player_ids[0]);
    assert_eq!(engine.state.priority_player_id(), player_ids[0]);

    let narset = prepared_narset_on_battlefield(&mut engine);
    engine
        .state
        .objects
        .get_mut(&narset)
        .expect("the actual registered Narset is on the battlefield")
        .set_counter(CounterKind::Loyalty, 9);
    inject_card_into_hand(&mut engine, 0, "divination");
    inject_card_into_hand(&mut engine, 1, "opt");
    inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 2, "divination");
    inject_card_into_hand(&mut engine, 2, "ornithopter");
    engine.initial_response_batch();
    (engine, player_ids, narset)
}

fn narset_resolve_actual_minus_nine(engine: &mut GameEngine, controller: i32, narset: u32) {
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.active_player_id(), controller);
    assert_eq!(engine.state.priority_player_id(), controller);
    assert_eq!(
        engine.state.objects[&narset].counter_count(CounterKind::Loyalty),
        9,
        "the prepared actual Narset has exactly nine loyalty"
    );
    let (source_generation, ability_index, activatable, activation) =
        actual_ability_command(engine, controller, narset, 2);
    assert_eq!(ability_index, 2);
    assert!(activatable, "the current actual -9 ability is offered");
    engine
        .apply_scenario_command(controller, &activation)
        .expect("activate actual Narset -9 from its current receipt");
    assert_eq!(engine.state.objects[&narset].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&narset].owner, controller);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&narset)
            .copied()
            .expect("Narset's graveyard incarnation has a generation"),
        source_generation + 1,
        "paying all nine loyalty puts the actual source into its owner's graveyard"
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the actual -9 ability remains on the stack after its source leaves"
    );
    narset_capture_priority_round(engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.static_emblems.len(),
        1,
        "the source-independent ability resolves one emblem"
    );
    let emblem = &engine.state.static_emblems[0];
    assert_eq!(emblem.controller, controller);
    assert!(emblem.effects.iter().any(|effect| matches!(
        effect,
        tricerules_card_model::primitives::StaticEmblemEffect::OpponentsCantCastNoncreatureSpells
    )));
}

fn narset_prepare_three_player_main1_priority(
    engine: &mut GameEngine,
    active_seat: usize,
    priority_seat: usize,
    turn: u32,
) {
    assert!(
        engine.state.stack.is_empty(),
        "prepared Main1 priority has an empty stack"
    );
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.blocking_choice().is_none());

    // These explicit Main1/priority assignments isolate each actual PlayerId's cast legality.
    engine.state.active_player_idx = active_seat;
    engine.state.priority_idx = priority_seat;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.turn = turn;
    engine.state.turn_instance = u64::from(turn);
    engine.state.passes_since_stack_change = 0;
    engine.state.lands_played_this_turn = 0;
}

fn narset_hand_cast_is_offered(engine: &mut GameEngine, player: i32, hand_index: usize) -> bool {
    engine
        .initial_response_batch()
        .legal_by_player
        .get(&player)
        .is_some_and(|actions| {
            actions.hand_actions.iter().any(|action| {
                action.kind == rv1::HandActionKind::HandActionCastSpell as i32
                    && action.hand_index as usize == hand_index
                    && action.face_index == 0
            })
        })
}

fn narset_begin_actual_hand_spell(
    engine: &GameEngine,
    player_seat: usize,
    card_id: &str,
) -> (u32, RuledCommand) {
    let hand_index = hand_index_for_card(engine, player_seat, card_id);
    let object_id = engine.state.players[player_seat].hand[hand_index];
    let cast = match cast_spell(hand_index, Vec::new()).cmd {
        Some(rv1::ruled_command::Cmd::CastSpell(cast)) => cast,
        _ => panic!("the actual hand spell builder returns CastSpell"),
    };
    assert_eq!(cast.face_index, 0);
    (
        object_id,
        RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
                rv1::BeginSpellCast {
                    announcement: Some(spell_announcement(cast)),
                },
            )),
        },
    )
}

fn narset_preview_current_hand_payment(
    engine: &GameEngine,
    caster: i32,
    transaction_id: u64,
    mana: rv1::PaymentMana,
    expected_cost: &str,
) -> rv1::PaymentPreview {
    let pending = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the actual hand spell has a current pending payment");
    assert_eq!(pending.transaction_id, transaction_id);
    assert_eq!(pending.caster, caster);
    let reserved_object_id = pending.reserved_object_id;
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&reserved_object_id)
        .copied()
        .unwrap_or(0);
    let revision = engine.state.command_index;
    let caster_index = engine
        .state
        .player_idx(caster)
        .expect("the actual spell caster remains in the game");
    let mana_before_preview = engine.state.players[caster_index].mana_pool;
    let preview = engine.preview_payment_scenario(
        caster,
        &rv1::PreviewPayment {
            transaction_id,
            revision,
            commit_spell_cast: Some(rv1::CommitSpellCast {
                transaction_id,
                payment: Some(rv1::PaymentSelection {
                    expected_state_revision: revision,
                    source: Some(rv1::CostObjectRef {
                        object_id: reserved_object_id,
                        zone_change_generation: source_generation,
                    }),
                    mana: Some(mana),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(
        preview.valid && preview.complete,
        "the independent current payment is legal: {preview:?}"
    );
    assert_eq!(preview.total_cost, expected_cost);
    assert_eq!(engine.state.command_index, revision, "preview is read-only");
    assert_eq!(
        engine.state.players[caster_index].mana_pool, mana_before_preview,
        "preview does not debit mana"
    );
    let selection = preview
        .selection
        .as_ref()
        .expect("a complete payment preview returns its current selection");
    assert_eq!(selection.expected_state_revision, revision);
    match selection.mana.as_ref() {
        Some(selected_mana) => assert_eq!(selected_mana, &mana),
        None => assert_eq!(
            mana,
            rv1::PaymentMana::default(),
            "only a zero-mana cast may omit its mana selection"
        ),
    }
    let source = selection
        .source
        .as_ref()
        .expect("the current physical spell receipt is in the payment selection");
    assert_eq!(source.object_id, reserved_object_id);
    assert_eq!(source.zone_change_generation, source_generation);
    preview
}

fn narset_commit_current_hand_payment(
    engine: &mut GameEngine,
    caster: i32,
    transaction_id: u64,
    preview: rv1::PaymentPreview,
) -> rv1::RuledEventBatch {
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id,
                        payment: preview.selection,
                        restricted_mana: preview.restricted_mana,
                    },
                )),
            },
        )
        .expect("commit the exact returned hand spell payment")
}

fn narset_assert_noncreature_unrestricted_control(
    engine: &mut GameEngine,
    player_seat: usize,
    card_id: &str,
    turn: u32,
    mana_gift: ManaGift,
    payment: rv1::PaymentMana,
    expected_cost: &str,
) {
    narset_prepare_three_player_main1_priority(engine, player_seat, player_seat, turn);
    let player = engine.state.players[player_seat].id;
    give_mana(engine, player, mana_gift);
    let (object_id, begin) = narset_begin_actual_hand_spell(engine, player_seat, card_id);
    let hand_index = hand_index_for_card(engine, player_seat, card_id);
    assert!(
        narset_hand_cast_is_offered(engine, player, hand_index),
        "the no-emblem control offers the otherwise-legal {card_id} cast"
    );
    let hand_before = engine.state.players[player_seat].hand.clone();
    let mana_before = engine.state.players[player_seat].mana_pool;
    engine
        .apply_scenario_command(player, &begin)
        .expect("the independent no-emblem control begins the real spell");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the control begins a pending payment")
        .transaction_id;
    let _preview =
        narset_preview_current_hand_payment(engine, player, transaction_id, payment, expected_cost);
    engine
        .apply_scenario_command(
            player,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                    rv1::CancelSpellCast { transaction_id },
                )),
            },
        )
        .expect("cancel the independent legality control after a complete preview");
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.players[player_seat].hand, hand_before);
    assert_eq!(engine.state.players[player_seat].mana_pool, mana_before);
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Hand);
}

fn narset_reject_opponent_noncreature_atomically(
    engine: &mut GameEngine,
    player_seat: usize,
    card_id: &str,
    turn: u32,
    mana_gift: ManaGift,
) {
    narset_prepare_three_player_main1_priority(engine, player_seat, player_seat, turn);
    let player = engine.state.players[player_seat].id;
    assert_ne!(player, engine.state.static_emblems[0].controller);
    give_mana(engine, player, mana_gift);
    let (object_id, begin) = narset_begin_actual_hand_spell(engine, player_seat, card_id);
    let hand_index = hand_index_for_card(engine, player_seat, card_id);
    assert!(
        !narset_hand_cast_is_offered(engine, player, hand_index),
        "the actual emblem removes the current {card_id} cast offer"
    );
    let hand_before = engine.state.players[player_seat].hand.clone();
    let mana_before = engine.state.players[player_seat].mana_pool;
    let zone_before = engine.state.objects[&object_id].zone;
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    let command_index_before = engine.state.command_index;
    let snapshot_before = full_snapshot(engine);
    let state_before = engine
        .diagnostic_snapshot()
        .expect("serialize diagnostics before forbidden Begin");

    let rejected = engine
        .apply_scenario_command(player, &begin)
        .expect_err("the opposing emblem blocks the noncreature BeginSpellCast");
    let reason = format!("{rejected:?}");
    assert!(
        reason.contains("an emblem prohibits this opponent from casting noncreature spells"),
        "the rejection comes from Narset's emblem restriction: {reason}"
    );
    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(engine.state.players[player_seat].hand, hand_before);
    assert_eq!(engine.state.players[player_seat].mana_pool, mana_before);
    assert_eq!(engine.state.objects[&object_id].zone, zone_before);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
        generation_before
    );
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(full_snapshot(engine), snapshot_before);
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("serialize diagnostics after forbidden Begin"),
        state_before,
        "the blocked current Begin preserves complete state"
    );
}

#[allow(clippy::too_many_arguments)]
fn narset_cast_allowed_spell(
    engine: &mut GameEngine,
    player_seat: usize,
    card_id: &str,
    turn: u32,
    mana_gift: ManaGift,
    payment: rv1::PaymentMana,
    expected_cost: &str,
    expected_zone: Zone,
    expected_draws: usize,
) -> u32 {
    narset_prepare_three_player_main1_priority(engine, player_seat, player_seat, turn);
    let player = engine.state.players[player_seat].id;
    give_mana(engine, player, mana_gift);
    let (object_id, begin) = narset_begin_actual_hand_spell(engine, player_seat, card_id);
    let hand_index = hand_index_for_card(engine, player_seat, card_id);
    assert!(
        narset_hand_cast_is_offered(engine, player, hand_index),
        "the current player is offered the actual {card_id} cast"
    );
    engine
        .apply_scenario_command(player, &begin)
        .expect("the current legal cast begins under Narset's emblem");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the allowed actual spell opens a payment")
        .transaction_id;
    let preview =
        narset_preview_current_hand_payment(engine, player, transaction_id, payment, expected_cost);
    let committed = narset_commit_current_hand_payment(engine, player, transaction_id, preview);
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Stack);
    let spell = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == object_id)
        .expect("the allowed actual card is committed as a spell");
    assert_eq!(spell.card_id, card_id);
    assert_eq!(spell.cast_by, Some(player));
    assert_eq!(
        spell.cast_method,
        tricerules_core::state::SpellCastMethod::Normal
    );
    let occurrence = spell
        .cast_occurrence
        .expect("the allowed actual spell has its physical cast occurrence");
    assert_eq!(
        committed
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(Ev::StackPushed(pushed)) if pushed.object_id == object_id
                )
            })
            .count(),
        1
    );
    let stack_generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .expect("the committed actual spell on the stack has a generation");
    assert_eq!(
        occurrence.zone_change_generation,
        Some(stack_generation),
        "the committed cast receipt matches the actual stack incarnation"
    );
    let hand_before_resolution = engine.state.players[player_seat].hand.len();
    narset_capture_priority_round(engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&object_id].zone, expected_zone);
    if expected_draws > 0 {
        assert_eq!(
            engine.state.players[player_seat].hand.len(),
            hand_before_resolution + expected_draws
        );
    } else {
        assert_eq!(
            engine.state.players[player_seat].hand.len(),
            hand_before_resolution
        );
    }
    if expected_zone == Zone::Battlefield {
        assert_eq!(engine.state.objects[&object_id].controller, player);
    }
    let final_generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .expect("the actual spell's final zone has a current generation");
    assert_eq!(
        final_generation,
        stack_generation + 1,
        "the actual spell leaves the stack in its next zone-change generation"
    );
    assert_eq!(occurrence.object_id, object_id);
    object_id
}

#[test]
fn narset_minus_nine_creates_a_persistent_three_player_prohibition() {
    const SEED: u64 = 26_110_241;

    let (mut control, control_ids, control_narset) = narset_three_player_ultimate_fixture(SEED);
    let (mut restricted, player_ids, narset) = narset_three_player_ultimate_fixture(SEED);
    assert_eq!(control_ids, player_ids);
    assert_eq!(control_narset, narset);
    assert_ne!(player_ids[0], player_ids[1]);
    assert_ne!(player_ids[0], player_ids[2]);
    assert_ne!(player_ids[1], player_ids[2]);
    for (seat, card_id) in [
        (0, "divination"),
        (1, "opt"),
        (1, "grizzly_bears"),
        (2, "divination"),
        (2, "ornithopter"),
    ] {
        let control_index = hand_index_for_card(&control, seat, card_id);
        let restricted_index = hand_index_for_card(&restricted, seat, card_id);
        assert_eq!(
            control.state.players[seat].hand[control_index],
            restricted.state.players[seat].hand[restricted_index],
            "the control and restricted fixtures begin with the same physical {card_id}"
        );
    }

    let controller = player_ids[0];
    let source_generation = restricted
        .state
        .zone_change_generation
        .get(&narset)
        .copied()
        .unwrap_or(0);
    narset_resolve_actual_minus_nine(&mut restricted, controller, narset);
    assert_eq!(
        restricted.state.zone_change_generation[&narset],
        source_generation + 1,
        "the ability receipt's source incarnation departed before resolution"
    );
    assert_eq!(
        restricted.state.objects[&narset].zone,
        Zone::Graveyard,
        "the one resolved emblem outlives Narset's actual source"
    );

    // Each opponent acts with priority during a different prepared Main1. The paired control
    // begins and completely previews the same actual noncreature spell with the same mana.
    narset_assert_noncreature_unrestricted_control(
        &mut control,
        1,
        "opt",
        2,
        ManaGift {
            u: 1,
            ..Default::default()
        },
        rv1::PaymentMana {
            u: 1,
            ..Default::default()
        },
        "{U}",
    );
    narset_reject_opponent_noncreature_atomically(
        &mut restricted,
        1,
        "opt",
        2,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    narset_assert_noncreature_unrestricted_control(
        &mut control,
        2,
        "divination",
        3,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        rv1::PaymentMana {
            u: 1,
            c: 2,
            ..Default::default()
        },
        "{2}{U}",
    );
    narset_reject_opponent_noncreature_atomically(
        &mut restricted,
        2,
        "divination",
        3,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    let controller_divination = narset_cast_allowed_spell(
        &mut restricted,
        0,
        "divination",
        4,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        rv1::PaymentMana {
            u: 1,
            c: 2,
            ..Default::default()
        },
        "{2}{U}",
        Zone::Graveyard,
        2,
    );
    assert_eq!(
        restricted.state.objects[&controller_divination].card_id,
        "divination"
    );
    narset_cast_allowed_spell(
        &mut restricted,
        1,
        "grizzly_bears",
        5,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        rv1::PaymentMana {
            g: 1,
            c: 1,
            ..Default::default()
        },
        "{1}{G}",
        Zone::Battlefield,
        0,
    );
    narset_cast_allowed_spell(
        &mut restricted,
        2,
        "ornithopter",
        6,
        ManaGift::default(),
        rv1::PaymentMana::default(),
        "{0}",
        Zone::Battlefield,
        0,
    );
    assert_eq!(restricted.state.objects[&narset].zone, Zone::Graveyard);
    assert_eq!(restricted.state.static_emblems.len(), 1);
    assert_eq!(restricted.state.static_emblems[0].controller, controller);
}
