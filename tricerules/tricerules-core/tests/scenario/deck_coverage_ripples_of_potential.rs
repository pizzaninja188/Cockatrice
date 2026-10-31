//! Actual-card scenarios for Ripples of Potential.
//! CR 701.34, 702.26g–i, and 608.2m govern these receipt-scoped choices and phasing.

use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::state::PhasedOutParentRef;
use tricerules_core::{AttachmentRecipient, Zone};
use tricerules_proto::ruled::v1 as rv1;

const RIPPLE: &str = "ripples_of_potential";
const CONTROLLER: i32 = 10;
const OPPONENT: i32 = 20;
const WITNESS: i32 = 30;

fn setup(seed: u64) -> GameEngine {
    setup_with_players(seed, &[CONTROLLER, OPPONENT])
}

fn setup_with_players(seed: u64, player_ids: &[i32]) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(RIPPLE)
        .expect("Ripples of Potential is registered before its actual-card scenario runs");
    assert_eq!(card.name, "Ripples of Potential");
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        player_ids,
        20,
        Some((0..player_ids.len()).map(|_| deck.clone()).collect()),
        true,
    )
    .expect("actual Ripples scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn proliferate_choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: 0,
                chosen_object_ids: objects,
                chosen_player_ids: players,
                decision: rv1::ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            },
        )),
    }
}

fn zone_change_generation(engine: &GameEngine, object: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object)
        .copied()
        .unwrap_or(0)
}

fn set_player_counter(engine: &mut GameEngine, player: i32, kind: CounterKind, count: u32) {
    let player_index = engine.state.player_idx(player).expect("player exists");
    engine.state.players[player_index]
        .counters
        .insert(kind, count);
}

fn cast_ripples(engine: &mut GameEngine) -> u32 {
    let spell = inject_card_into_hand(engine, 0, RIPPLE);
    grant_pool(engine, 0);
    let hand_index = hand_index_for_card(engine, 0, RIPPLE);
    engine
        .apply_scenario_command(CONTROLLER, &cast_spell(hand_index, vec![]))
        .expect("cast the registered Ripples of Potential");
    pass_priority_round(engine);
    spell
}

fn start_ripples(engine: &mut GameEngine) -> u32 {
    let spell = cast_ripples(engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Program Proliferate asks for its receipt recipients")
            .presentation
            .choice_kind,
        rv1::ChoiceKind::Proliferate
    );
    spell
}

/// Fixture-only ownership divergence for multiplayer departure coverage. The actual spell is
/// cast normally, then its physical owner is changed while it is on the stack so the test can
/// distinguish source-owner departure from controller departure.
fn start_ripples_with_fixture_owner(
    engine: &mut GameEngine,
    caster_index: usize,
    physical_owner: i32,
) -> u32 {
    let spell = inject_card_into_hand(engine, caster_index, RIPPLE);
    grant_pool(engine, caster_index);
    let hand_index = hand_index_for_card(engine, caster_index, RIPPLE);
    let caster = engine.state.players[caster_index].id;
    engine
        .apply_scenario_command(caster, &cast_spell(hand_index, vec![]))
        .expect("cast the registered Ripples of Potential before fixture ownership diverges");
    assert_eq!(
        engine.state.stack.last().map(|item| item.id),
        Some(spell),
        "the actual Ripples spell is on the stack before its physical owner is adjusted"
    );
    engine
        .state
        .objects
        .get_mut(&spell)
        .expect("the actual spell object remains on the stack")
        .owner = physical_owner;
    engine
        .state
        .stack
        .last_mut()
        .expect("the actual spell item remains on the stack")
        .source_owner = Some(physical_owner);
    pass_priority_round(engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the actual spell begins its Program Proliferate choice")
            .presentation
            .choice_kind,
        rv1::ChoiceKind::Proliferate
    );
    spell
}

fn ripples_permanent_choice_at(prompt_id: u64, chosen: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: prompt_id,
                chosen_object_ids: chosen,
                decision: rv1::ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            },
        )),
    }
}

fn assert_ripples_choice_rejected_atomically(
    engine: &mut GameEngine,
    actor: i32,
    command: &RuledCommand,
) {
    let state_before = format!("{:?}", engine.state);
    let command_index_before = engine.state.command_index;
    assert!(
        engine.apply_command(actor, command).is_err(),
        "invalid Ripples choice must be rejected"
    );
    assert_eq!(format!("{:?}", engine.state), state_before);
    assert_eq!(engine.state.command_index, command_index_before);
}

fn assert_ripples_generations_unchanged(engine: &GameEngine, expected: &[(u32, u64)]) {
    for (object, generation) in expected {
        assert_eq!(zone_change_generation(engine, *object), *generation);
    }
}

fn ripples_stack_resolved_count(batch: &rv1::RuledEventBatch, stack_id: u32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(rv1::ruled_event::Ev::StackResolved(resolved))
                    if resolved.object_id == stack_id
            )
        })
        .count()
}

fn ripples_resolution_log_count(batch: &rv1::RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(rv1::ruled_event::Ev::Log(log))
                    if log.text == "Ripples of Potential resolves."
            )
        })
        .count()
}

fn resolve_ripples_proliferate(
    engine: &mut GameEngine,
    objects: Vec<u32>,
    players: Vec<i32>,
) -> rv1::RuledEventBatch {
    engine
        .apply_scenario_command(CONTROLLER, &proliferate_choice(objects, players))
        .expect("resolve Ripples' Program Proliferate choice")
}

fn choose_ripples_permanents(engine: &mut GameEngine, chosen: Vec<u32>) {
    engine
        .apply_scenario_command(CONTROLLER, &submit_resolution_choice(chosen))
        .expect("submit Ripples' non-targeting permanent choice");
}

#[test]
fn ripples_proliferates_then_phases_only_selected_receipted_permanents() {
    let mut engine = setup(20_261_031);
    set_player_counter(&mut engine, OPPONENT, CounterKind::Poison, 1);
    let own_creature = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let own_artifact = inject_permanent_on_battlefield(&mut engine, 0, "everflowing_chalice");
    let opponent_creature = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let unreceipted_creature = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    for (object, counter) in [
        (own_creature, CounterKind::PlusOnePlusOne),
        (own_artifact, CounterKind::Charge),
        (opponent_creature, CounterKind::PlusOnePlusOne),
    ] {
        engine
            .state
            .objects
            .get_mut(&object)
            .expect("fixture permanent")
            .set_counter(counter, 1);
    }
    let own_creature_generation = zone_change_generation(&engine, own_creature);
    let own_artifact_generation = zone_change_generation(&engine, own_artifact);
    let opponent_creature_generation = zone_change_generation(&engine, opponent_creature);
    let unreceipted_generation = zone_change_generation(&engine, unreceipted_creature);

    let spell = start_ripples(&mut engine);

    let first_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Program Proliferate asks for its receipt recipients");
    assert_eq!(first_choice.deciding_player, CONTROLLER);
    assert_eq!(
        first_choice.presentation.choice_kind,
        rv1::ChoiceKind::Proliferate
    );
    assert!(first_choice.presentation.candidates.contains(&own_creature));
    assert!(first_choice.presentation.candidates.contains(&own_artifact));
    assert!(first_choice
        .presentation
        .candidates
        .contains(&opponent_creature));
    assert!(!first_choice
        .presentation
        .candidates
        .contains(&unreceipted_creature));

    let second_batch = resolve_ripples_proliferate(
        &mut engine,
        vec![own_creature, own_artifact, opponent_creature],
        vec![OPPONENT],
    );
    let opponent_index = engine.state.player_idx(OPPONENT).expect("opponent exists");
    assert_eq!(
        engine.state.players[opponent_index].counters[&CounterKind::Poison],
        2,
        "choosing a player in Proliferate increments their counter"
    );
    let second_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ripples asks for a non-targeting subset after placements complete");
    assert_eq!(second_choice.deciding_player, CONTROLLER);
    assert_eq!(
        second_choice.presentation.choice_kind,
        rv1::ChoiceKind::PermanentObjects
    );
    assert_eq!(second_choice.presentation.min, 0);
    assert_eq!(second_choice.presentation.max, 2);
    assert_eq!(
        second_choice.presentation.candidates,
        vec![own_creature, own_artifact],
        "the second choice uses positive receipts but excludes current opponent control"
    );
    let second_offer = second_batch
        .events
        .into_iter()
        .find_map(|event| match event.ev {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == rv1::ChoiceKind::PermanentObjects as i32 =>
            {
                Some(choice)
            }
            _ => None,
        })
        .expect("the choice menu is published with object candidates only");
    assert_eq!(second_offer.candidate_player_ids, Vec::<i32>::new());
    assert!(!second_offer
        .candidate_object_ids
        .contains(&opponent_creature));
    assert!(!second_offer
        .candidate_object_ids
        .contains(&unreceipted_creature));

    let completed = engine
        .apply_scenario_command(CONTROLLER, &submit_resolution_choice(vec![own_creature]))
        .expect("the ordinary actual-card choice resolves the spell");
    assert_eq!(ripples_stack_resolved_count(&completed, spell), 1);
    assert_eq!(ripples_resolution_log_count(&completed), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects[&own_creature].phase_status.is_some());
    assert!(engine.state.objects[&own_artifact].phase_status.is_none());
    assert!(engine.state.objects[&opponent_creature]
        .phase_status
        .is_none());
    assert!(engine.state.objects[&unreceipted_creature]
        .phase_status
        .is_none());
    assert_eq!(
        engine.state.objects[&own_creature].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        engine.state.objects[&own_artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&opponent_creature].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(engine.state.objects[&own_creature].zone, Zone::Battlefield);
    assert_eq!(
        zone_change_generation(&engine, own_creature),
        own_creature_generation
    );
    assert_eq!(
        zone_change_generation(&engine, own_artifact),
        own_artifact_generation
    );
    assert_eq!(
        zone_change_generation(&engine, opponent_creature),
        opponent_creature_generation
    );
    assert_eq!(
        zone_change_generation(&engine, unreceipted_creature),
        unreceipted_generation
    );
}

#[test]
fn selected_attachment_phases_directly_and_returns_to_its_exact_recipient() {
    let mut engine = setup(20_261_032);
    let host = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("registered Aura")
        .attached_to = Some(AttachmentRecipient::Object(host));
    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("Aura with a receipt counter")
        .set_counter(CounterKind::Charge, 1);
    engine.initial_response_batch();
    let aura_generation = zone_change_generation(&engine, aura);
    let host_generation = zone_change_generation(&engine, host);
    let original_recipient = engine.state.objects[&aura].attached_to;

    start_ripples(&mut engine);
    resolve_ripples_proliferate(&mut engine, vec![aura], Vec::new());
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the successful Aura counter placement supplies one choice candidate");
    assert_eq!(choice.presentation.candidates, vec![aura]);
    assert_eq!((choice.presentation.min, choice.presentation.max), (0, 1));
    choose_ripples_permanents(&mut engine, vec![aura]);

    let status = engine.state.objects[&aura]
        .phase_status
        .as_ref()
        .expect("the chosen attachment phases out");
    assert!(status.phase_in_with.is_none());
    assert!(status.direct_attachment_snapshot.is_some());
    assert_eq!(status.phase_out_under, CONTROLLER);
    assert!(engine.state.objects[&host].phase_status.is_none());
    assert_eq!(engine.state.objects[&aura].attached_to, original_recipient);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(zone_change_generation(&engine, aura), aura_generation);
    assert_eq!(zone_change_generation(&engine, host), host_generation);
    assert_eq!(
        engine.state.objects[&aura].counter_count(CounterKind::Charge),
        2
    );

    end_active_turn(&mut engine, CONTROLLER);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, OPPONENT);
    advance_to_main1_from_game_start(&mut engine);
    assert!(engine.state.objects[&aura].phase_status.is_none());
    assert_eq!(engine.state.objects[&aura].attached_to, original_recipient);
    assert_eq!(zone_change_generation(&engine, aura), aura_generation);
    assert_eq!(zone_change_generation(&engine, host), host_generation);
    assert_eq!(
        engine.state.objects[&aura].counter_count(CounterKind::Charge),
        2
    );
}

#[test]
fn selected_parent_and_attachment_phase_as_one_indirect_cohort_in_either_order() {
    for (offset, selected_order) in [(0, [0_usize, 1_usize]), (1, [1_usize, 0_usize])] {
        let mut engine = setup(20_261_033 + offset);
        let parent = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
        let selected_aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
        let other_controller_aura = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
        for aura in [selected_aura, other_controller_aura] {
            engine
                .state
                .objects
                .get_mut(&aura)
                .expect("registered Aura")
                .attached_to = Some(AttachmentRecipient::Object(parent));
        }
        engine
            .state
            .objects
            .get_mut(&parent)
            .expect("receipt-bearing creature")
            .set_counter(CounterKind::PlusOnePlusOne, 1);
        engine
            .state
            .objects
            .get_mut(&selected_aura)
            .expect("receipt-bearing selected Aura")
            .set_counter(CounterKind::Charge, 1);
        engine.initial_response_batch();
        let parent_generation = zone_change_generation(&engine, parent);
        let selected_aura_generation = zone_change_generation(&engine, selected_aura);
        let other_aura_generation = zone_change_generation(&engine, other_controller_aura);

        start_ripples(&mut engine);
        resolve_ripples_proliferate(&mut engine, vec![parent, selected_aura], Vec::new());
        let offered = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the second choice opens over the two exact receipts");
        assert_eq!(offered.presentation.max, 2);
        assert!(offered.presentation.candidates.contains(&parent));
        assert!(offered.presentation.candidates.contains(&selected_aura));
        assert!(!offered
            .presentation
            .candidates
            .contains(&other_controller_aura));
        let selected = selected_order
            .into_iter()
            .map(|index| [parent, selected_aura][index])
            .collect();
        choose_ripples_permanents(&mut engine, selected);

        for member in [parent, selected_aura, other_controller_aura] {
            let status = engine.state.objects[&member]
                .phase_status
                .as_ref()
                .expect("all active attachments phase indirectly with the selected parent");
            assert_eq!(status.phase_out_under, CONTROLLER);
            assert!(status.direct_attachment_snapshot.is_none());
        }
        assert!(engine.state.objects[&parent]
            .phase_status
            .as_ref()
            .unwrap()
            .phase_in_with
            .is_none());
        for aura in [selected_aura, other_controller_aura] {
            assert_eq!(
                engine.state.objects[&aura]
                    .phase_status
                    .as_ref()
                    .unwrap()
                    .phase_in_with,
                Some(PhasedOutParentRef {
                    object_id: parent,
                    zone_change_generation: parent_generation,
                })
            );
            assert_eq!(
                engine.state.objects[&aura].attached_to,
                Some(AttachmentRecipient::Object(parent))
            );
        }
        assert_eq!(
            engine.state.objects[&parent].counter_count(CounterKind::PlusOnePlusOne),
            2
        );
        assert_eq!(
            engine.state.objects[&selected_aura].counter_count(CounterKind::Charge),
            2
        );
        assert_eq!(zone_change_generation(&engine, parent), parent_generation);
        assert_eq!(
            zone_change_generation(&engine, selected_aura),
            selected_aura_generation
        );
        assert_eq!(
            zone_change_generation(&engine, other_controller_aura),
            other_aura_generation
        );

        end_active_turn(&mut engine, CONTROLLER);
        advance_to_main1_from_game_start(&mut engine);
        end_active_turn(&mut engine, OPPONENT);
        advance_to_main1_from_game_start(&mut engine);
        for member in [parent, selected_aura, other_controller_aura] {
            assert!(engine.state.objects[&member].phase_status.is_none());
            assert_eq!(
                zone_change_generation(&engine, member),
                match member {
                    id if id == parent => parent_generation,
                    id if id == selected_aura => selected_aura_generation,
                    _ => other_aura_generation,
                }
            );
        }
        assert_eq!(
            engine.state.objects[&selected_aura].attached_to,
            Some(AttachmentRecipient::Object(parent))
        );
        assert_eq!(
            engine.state.objects[&other_controller_aura].attached_to,
            Some(AttachmentRecipient::Object(parent))
        );
    }
}

#[test]
fn zero_receipts_complete_without_opening_a_permanent_choice() {
    let mut engine = setup(20_261_035);
    let untouched = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let spell = cast_ripples(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.objects[&untouched].phase_status.is_none());
    assert_eq!(
        engine
            .state
            .objects
            .get(&untouched)
            .expect("untouched permanent")
            .counters
            .len(),
        0
    );
}

#[test]
fn nonempty_receipt_offer_allows_choosing_zero() {
    let mut engine = setup(20_261_041);
    let creature = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("receipt-bearing permanent")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let spell = start_ripples(&mut engine);

    resolve_ripples_proliferate(&mut engine, vec![creature], Vec::new());
    let offered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("a positive receipt opens a nonempty second-choice domain");
    assert_eq!(
        offered.presentation.choice_kind,
        rv1::ChoiceKind::PermanentObjects
    );
    assert_eq!(offered.presentation.candidates, vec![creature]);
    assert_eq!((offered.presentation.min, offered.presentation.max), (0, 1));
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        2
    );

    let completed = engine
        .apply_scenario_command(CONTROLLER, &submit_resolution_choice(Vec::new()))
        .expect("the player may choose zero from a nonempty receipt domain");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects[&creature].phase_status.is_none());
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "declining the optional subset does not undo Proliferate"
    );
    assert_eq!(ripples_stack_resolved_count(&completed, spell), 1);
}

#[test]
fn multiple_counter_kinds_and_doubling_produce_one_receipt_choice() {
    let mut engine = setup(20_261_036);
    let creature = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("receipt-bearing permanent")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("the same permanent has a second counter kind")
        .set_counter(CounterKind::Charge, 1);

    start_ripples(&mut engine);
    resolve_ripples_proliferate(&mut engine, vec![creature], Vec::new());
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("positive doubled placements produce the specialized choice");
    assert_eq!(choice.presentation.candidates, vec![creature]);
    assert_eq!((choice.presentation.min, choice.presentation.max), (0, 1));
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        3
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Charge),
        3
    );
    choose_ripples_permanents(&mut engine, vec![creature]);
    assert!(engine.state.objects[&creature].phase_status.is_some());
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        3
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Charge),
        3
    );
}

#[test]
fn all_receipted_artifact_land_and_planeswalker_are_selectable() {
    let mut engine = setup(20_261_042);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "everflowing_chalice");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let planeswalker = inject_permanent_on_battlefield(&mut engine, 0, "teferi,_temporal_archmage");
    for (object, kind) in [
        (artifact, CounterKind::Charge),
        (land, CounterKind::Charge),
        (planeswalker, CounterKind::Loyalty),
    ] {
        engine
            .state
            .objects
            .get_mut(&object)
            .expect("actual permanent fixture")
            .set_counter(kind, 1);
    }
    let generations = [
        (artifact, zone_change_generation(&engine, artifact)),
        (land, zone_change_generation(&engine, land)),
        (planeswalker, zone_change_generation(&engine, planeswalker)),
    ];
    let spell = start_ripples(&mut engine);

    resolve_ripples_proliferate(&mut engine, vec![artifact, land, planeswalker], Vec::new());
    let offered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("each actual permanent kind received a positive placement");
    assert_eq!(
        offered.presentation.choice_kind,
        rv1::ChoiceKind::PermanentObjects
    );
    assert_eq!(
        offered.presentation.candidates,
        vec![artifact, land, planeswalker]
    );
    assert_eq!((offered.presentation.min, offered.presentation.max), (0, 3));
    assert_eq!(
        engine.state.objects[&artifact].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&land].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&planeswalker].counter_count(CounterKind::Loyalty),
        2
    );

    let completed = engine
        .apply_scenario_command(
            CONTROLLER,
            &submit_resolution_choice(vec![artifact, land, planeswalker]),
        )
        .expect("all three actual permanent types can be selected");
    assert_eq!(ripples_stack_resolved_count(&completed, spell), 1);
    for (object, kind) in [
        (artifact, CounterKind::Charge),
        (land, CounterKind::Charge),
        (planeswalker, CounterKind::Loyalty),
    ] {
        assert!(engine.state.objects[&object].phase_status.is_some());
        assert_eq!(engine.state.objects[&object].counter_count(kind), 2);
    }
    for (object, generation) in generations {
        assert_eq!(zone_change_generation(&engine, object), generation);
    }
}

#[test]
fn suspended_counter_replacement_resumes_proliferate_before_opening_phase_choice() {
    let mut engine = setup(20_261_043);
    let creature = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let doublers = [
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season"),
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season"),
    ];
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("receipt-bearing permanent")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let spell = start_ripples(&mut engine);

    let replacement_batch = resolve_ripples_proliferate(&mut engine, vec![creature], Vec::new());
    let replacement = find_resolution_choice(&replacement_batch)
        .expect("two actual Doubling Seasons suspend one Proliferate placement for ordering");
    assert_eq!(
        replacement.choice_kind,
        rv1::ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(replacement.deciding_player_id, CONTROLLER);
    assert_eq!(replacement.replacement_options.len(), 2);
    assert!(replacement
        .replacement_options
        .iter()
        .all(|option| doublers.contains(&option.source_object_id)));

    let after_order = engine
        .apply_scenario_command(
            CONTROLLER,
            &submit_resolution_choice(vec![replacement.candidate_object_ids[0]]),
        )
        .expect("the counter-order answer resumes the parked actual-card Program");
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        5,
        "both Doubling Seasons apply to the single Proliferate counter"
    );
    let phase_choice = find_resolution_choice(&after_order)
        .expect("the completed placement creates the sealed second choice");
    assert_eq!(
        phase_choice.choice_kind,
        rv1::ChoiceKind::PermanentObjects as i32
    );
    assert_eq!(phase_choice.deciding_player_id, CONTROLLER);
    assert_eq!(phase_choice.candidate_object_ids, vec![creature]);
    assert_eq!((phase_choice.min, phase_choice.max), (0, 1));

    let completed = engine
        .apply_scenario_command(CONTROLLER, &submit_resolution_choice(vec![creature]))
        .expect("the second choice phases the exact recipient after replacement resumption");
    assert_eq!(ripples_stack_resolved_count(&completed, spell), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects[&creature].phase_status.is_some());
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        5,
        "phasing preserves the post-replacement counter total"
    );
}

#[test]
fn prevented_counter_placements_do_not_enter_the_phase_out_cohort() {
    let mut engine = setup(20_261_037);
    let creature = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 1, "vorinclex,_monstrous_raider");
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("counter-bearing permanent")
        .set_counter(CounterKind::PlusOnePlusOne, 1);

    start_ripples(&mut engine);
    resolve_ripples_proliferate(&mut engine, vec![creature], Vec::new());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert!(engine.state.objects[&creature].phase_status.is_none());
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the opponent's Vorinclex reduces the attempted one-counter placement to zero"
    );
}

#[test]
fn proliferated_permanent_answers_reject_duplicate_foreign_nonreceipt_and_stale_generation_atomically(
) {
    let mut engine = setup(20_261_038);
    let own = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_own = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let foreign = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let unreceipted = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    for object in [own, second_own, foreign] {
        engine
            .state
            .objects
            .get_mut(&object)
            .expect("receipt-bearing permanent")
            .set_counter(CounterKind::PlusOnePlusOne, 1);
    }

    let spell = start_ripples(&mut engine);
    resolve_ripples_proliferate(&mut engine, vec![own, second_own, foreign], Vec::new());
    let offered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("successful controller-owned placements open the second choice");
    assert_eq!(offered.presentation.candidates, vec![own, second_own]);
    assert_eq!(
        (offered.presentation.min, offered.presentation.max),
        (0, 2),
        "two eligible receipt candidates make a duplicate pair fit the offered maximum"
    );
    let prompt_id = offered.resolution_prompt_id.get();
    assert_ne!(prompt_id, 0);
    let captured_generations = [
        (own, zone_change_generation(&engine, own)),
        (second_own, zone_change_generation(&engine, second_own)),
        (foreign, zone_change_generation(&engine, foreign)),
    ];

    assert_ripples_choice_rejected_atomically(
        &mut engine,
        CONTROLLER,
        &ripples_permanent_choice_at(prompt_id, vec![own, own]),
    );
    assert_ripples_generations_unchanged(&engine, &captured_generations);
    assert_ripples_choice_rejected_atomically(
        &mut engine,
        CONTROLLER,
        &ripples_permanent_choice_at(prompt_id, vec![foreign]),
    );
    assert_ripples_generations_unchanged(&engine, &captured_generations);
    assert_ripples_choice_rejected_atomically(
        &mut engine,
        CONTROLLER,
        &ripples_permanent_choice_at(prompt_id, vec![unreceipted]),
    );
    assert_ripples_generations_unchanged(&engine, &captured_generations);

    // Prepared-state defensive probe: control changes without a zone transition while this
    // prompt is parked are not caused by ordinary priority in this fixture.
    let original_controller = engine.state.players[0].id;
    let new_controller = engine.state.players[1].id;
    assert_eq!(original_controller, CONTROLLER);
    assert_eq!(new_controller, OPPONENT);
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != own);
    engine.state.players[1].battlefield.push(own);
    let changed = engine
        .state
        .objects
        .get_mut(&own)
        .expect("the already-offered permanent remains in the same incarnation");
    changed.base_controller = new_controller;
    changed.controller = new_controller;

    assert_eq!(
        zone_change_generation(&engine, own),
        captured_generations[0].1
    );
    let still_offered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original bounded choice remains parked");
    assert_eq!(still_offered.resolution_prompt_id.get(), prompt_id);
    assert_eq!(still_offered.deciding_player, CONTROLLER);
    assert_eq!(still_offered.presentation.candidates, vec![own, second_own]);
    let offered_own_generation = match &still_offered.continuation {
        tricerules_core::state::ResolutionContinuation::PermanentChoice {
            candidate_generations,
            ..
        } => candidate_generations
            .iter()
            .find_map(|(object_id, generation)| (*object_id == own).then_some(*generation)),
        _ => None,
    };
    assert_eq!(offered_own_generation, Some(captured_generations[0].1));
    assert_eq!(engine.state.objects[&own].controller, new_controller);
    assert!(!engine.state.players[0].battlefield.contains(&own));
    assert!(engine.state.players[1].battlefield.contains(&own));
    assert_ripples_choice_rejected_atomically(
        &mut engine,
        CONTROLLER,
        &ripples_permanent_choice_at(prompt_id, vec![own]),
    );
    assert_ripples_generations_unchanged(&engine, &captured_generations);

    // Restore the prepared-state controller/index so the existing stale-generation case still
    // reaches its intended guard rather than being rejected by current control first.
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != own);
    engine.state.players[0].battlefield.push(own);
    let restored = engine
        .state
        .objects
        .get_mut(&own)
        .expect("the permanent remains available for the stale-generation probe");
    restored.base_controller = original_controller;
    restored.controller = original_controller;

    // Simulate the same ObjectId leaving and returning while the captured receipt still names
    // its prior zone-change generation. A matching current battlefield ID cannot reuse that
    // stale receipt.
    let returned_generation = captured_generations[0].1 + 2;
    engine
        .state
        .zone_change_generation
        .insert(own, returned_generation);
    assert_ripples_choice_rejected_atomically(
        &mut engine,
        CONTROLLER,
        &ripples_permanent_choice_at(prompt_id, vec![own]),
    );
    assert_ripples_generations_unchanged(
        &engine,
        &[
            (own, returned_generation),
            captured_generations[1],
            captured_generations[2],
        ],
    );

    let resolved = engine
        .apply_command(
            CONTROLLER,
            &ripples_permanent_choice_at(prompt_id, vec![second_own]),
        )
        .expect("the remaining current-controller candidate still completes the original offer");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&own].controller, original_controller);
    assert!(engine.state.objects[&own].phase_status.is_none());
    assert!(engine.state.objects[&second_own].phase_status.is_some());
    assert_eq!(ripples_resolution_log_count(&resolved), 1);
    assert_eq!(ripples_stack_resolved_count(&resolved, spell), 1);
}

#[test]
fn controller_departure_finishes_begun_ripples_selection_empty() {
    let mut engine = setup_with_players(20_261_039, &[CONTROLLER, OPPONENT, WITNESS]);
    let controller_owned = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let surviving_player_permanent =
        inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&controller_owned)
        .expect("the departed controller's permanent receives a counter")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let spell = start_ripples_with_fixture_owner(&mut engine, 0, OPPONENT);
    resolve_ripples_proliferate(&mut engine, vec![controller_owned], Vec::new());
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the positive placement begins the bounded permanent choice");
    assert_eq!(pending.deciding_player, CONTROLLER);
    assert_eq!(pending.presentation.candidates, vec![controller_owned]);
    assert_eq!(
        engine.state.stack.last().map(|item| item.id),
        None,
        "the resolving spell is parked in the choice continuation"
    );

    let departure = engine
        .apply_scenario_command(CONTROLLER, &concede())
        .expect("the controller may concede during the begun Ripples choice");
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(
        !engine.state.objects.contains_key(&controller_owned),
        "the departed controller's owned permanent leaves the game"
    );
    assert_eq!(
        engine.state.objects[&surviving_player_permanent].zone,
        Zone::Battlefield,
        "the surviving player's unrelated permanent remains in the game"
    );
    assert_eq!(engine.state.objects[&spell].owner, OPPONENT);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Exile);
    assert!(find_resolution_choice(&departure).is_none());
    assert_eq!(ripples_resolution_log_count(&departure), 1);
}

#[test]
fn owner_departure_prunes_receipts_and_reissues_only_to_the_surviving_controller() {
    let mut engine = setup_with_players(20_261_040, &[CONTROLLER, OPPONENT, WITNESS]);
    let departed_owner_candidate = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&departed_owner_candidate)
        .expect("fixture permanent is controlled by P0 but owned by P1")
        .owner = OPPONENT;
    let surviving_candidate = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    for object in [departed_owner_candidate, surviving_candidate] {
        engine
            .state
            .objects
            .get_mut(&object)
            .expect("both permanents receive a counter before owner departure")
            .set_counter(CounterKind::PlusOnePlusOne, 1);
    }

    let spell = start_ripples_with_fixture_owner(&mut engine, 0, OPPONENT);
    resolve_ripples_proliferate(
        &mut engine,
        vec![departed_owner_candidate, surviving_candidate],
        Vec::new(),
    );
    let initial = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the two successful placements open one exact-recipient choice");
    assert_eq!(initial.deciding_player, CONTROLLER);
    assert_eq!(
        initial.presentation.candidates,
        vec![departed_owner_candidate, surviving_candidate]
    );
    let original_prompt_id = initial.resolution_prompt_id.get();
    assert_ne!(original_prompt_id, 0);

    let departure = engine
        .apply_scenario_command(OPPONENT, &concede())
        .expect("the physical spell owner may depart while its controller's choice is open");
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.objects.contains_key(&spell));
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::StackObjectRemoved(removed))
                        if removed.object_id == spell
                )
            })
            .count(),
        1,
        "the departed physical spell is removed once while its begun effect continues"
    );
    let refreshed = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the surviving spell controller keeps the begun choice");
    assert_eq!(refreshed.deciding_player, CONTROLLER);
    assert_eq!(
        refreshed.presentation.choice_kind,
        rv1::ChoiceKind::PermanentObjects
    );
    assert_eq!(refreshed.presentation.candidates, vec![surviving_candidate]);
    assert_eq!(
        engine.state.objects[&surviving_candidate].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "refreshing the second choice does not replay the earlier counter placement"
    );
    let refreshed_prompt_id = refreshed.resolution_prompt_id.get();
    assert_ne!(refreshed_prompt_id, original_prompt_id);
    assert_eq!(
        find_resolution_choice(&departure)
            .expect("owner departure publishes the refreshed candidate domain")
            .resolution_prompt_id,
        refreshed_prompt_id
    );

    assert_ripples_choice_rejected_atomically(
        &mut engine,
        CONTROLLER,
        &ripples_permanent_choice_at(original_prompt_id, vec![departed_owner_candidate]),
    );
    let accepted = engine
        .apply_command(
            CONTROLLER,
            &ripples_permanent_choice_at(refreshed_prompt_id, vec![surviving_candidate]),
        )
        .expect("the reissued choice accepts the surviving receipted permanent");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects[&surviving_candidate]
        .phase_status
        .is_some());
    assert_eq!(
        engine.state.objects[&surviving_candidate].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "answering the reissued choice does not replay Proliferate"
    );
    assert_eq!(ripples_resolution_log_count(&accepted), 1);
}
