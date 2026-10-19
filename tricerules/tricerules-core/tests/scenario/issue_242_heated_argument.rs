use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    permanent_moved::Destination, ruled_command::Cmd, ruled_event::Ev, ChoiceKind,
    ResolutionChoiceDecision, RuledCommand, RuledEventBatch, SubmitResolutionChoice,
};

fn select_branch() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn resolve_top(engine: &mut GameEngine) -> RuledEventBatch {
    let mut batch = None;
    for _ in 0..engine.state.players.len() {
        batch = Some(
            engine
                .apply_scenario_command(engine.state.priority_player_id(), &pass())
                .expect("priority pass"),
        );
    }
    batch.expect("priority round batch")
}

fn choice_count(batch: &RuledEventBatch, choice_kind: ChoiceKind) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(Ev::ResolutionChoiceRequired(required))
                    if required.choice_kind == choice_kind as i32
            )
        })
        .count()
}

fn setup(seed: u64, with_graveyard_card: bool) -> (GameEngine, u32, Option<u32>) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["heated_argument"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Heated Argument is registered");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "heated_argument");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );
    let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    let graveyard = with_graveyard_card.then(|| inject_graveyard_card(&mut engine, 0, "mountain"));
    (engine, target, graveyard)
}

fn setup_sparse(seed: u64, with_graveyard_card: bool) -> (GameEngine, u32, Option<u32>) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[10, 20, 30],
        20,
        Some(vec![
            deck_with("mountain", &["heated_argument"]),
            forest_only_deck(),
            deck_with("island", &["ray_of_command"]),
        ]),
        true,
    )
    .expect("Heated Argument and Ray of Command are registered");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "heated_argument");
    ensure_card_in_hand(&mut engine, 2, "ray_of_command");
    give_mana(
        &mut engine,
        10,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        30,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    let graveyard = with_graveyard_card.then(|| inject_graveyard_card(&mut engine, 0, "mountain"));
    (engine, target, graveyard)
}

fn cast_and_park(engine: &mut GameEngine, target: u32) -> RuledEventBatch {
    let slot = hand_index_for_card(engine, 0, "heated_argument");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Heated Argument");
    let batch = resolve_top(engine);
    assert_eq!(engine.state.objects[&target].damage, 6);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("optional exile branch")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    batch
}

#[test]
fn issue_242_payment_exiles_exact_card_and_damages_current_controller_after_ray_response() {
    let (mut engine, target, graveyard) = setup_sparse(242_001, true);
    let graveyard = graveyard.unwrap();
    let heated_slot = hand_index_for_card(&engine, 0, "heated_argument");
    engine
        .apply_scenario_command(10, &cast_spell(heated_slot, target_object(target)))
        .expect("P10 casts Heated Argument at P20's creature");
    assert_eq!(engine.state.priority_player_id(), 10);
    engine
        .apply_scenario_command(10, &pass())
        .expect("the caster passes priority after casting Heated Argument");
    assert_eq!(engine.state.priority_player_id(), 20);
    engine
        .apply_scenario_command(20, &pass())
        .expect("P20 passes priority");
    assert_eq!(engine.state.priority_player_id(), 30);
    let ray_slot = hand_index_for_card(&engine, 2, "ray_of_command");
    engine
        .apply_scenario_command(30, &cast_spell(ray_slot, target_object(target)))
        .expect("P30 casts Ray of Command in response");
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&target].controller, 30);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Heated Argument remains below Ray"
    );

    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&target].damage, 6);
    let branch_prompt = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(branch_prompt.deciding_player, 10);
    assert_eq!(
        branch_prompt.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    engine.apply_scenario_command(10, &select_branch()).unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 10);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::GraveyardCards);
    assert_eq!(pending.presentation.candidates, vec![graveyard]);
    assert_eq!((pending.presentation.min, pending.presentation.max), (0, 1));

    let batch = engine
        .apply_scenario_command(10, &submit_resolution_choice(vec![graveyard]))
        .expect("pay by exiling the selected incarnation");
    assert_eq!(engine.state.objects[&graveyard].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[2].life, 18);
    assert_eq!(engine.state.objects[&target].owner, 20);
    assert_eq!(engine.state.objects[&target].controller, 30);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved))
            if moved.object_id == graveyard
                && moved.destination == Destination::Exile as i32
    )));
}

#[test]
fn issue_242_decline_and_empty_graveyard_skip_the_contingent_damage() {
    let (mut declined, target, graveyard) = setup(242_002, true);
    cast_and_park(&mut declined, target);
    declined
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .unwrap();
    assert_eq!(
        declined.state.objects[&graveyard.unwrap()].zone,
        Zone::Graveyard
    );
    assert_eq!(declined.state.players[1].life, 20);

    let (mut empty_payment, target, graveyard) = setup(242_004, true);
    let graveyard = graveyard.unwrap();
    cast_and_park(&mut empty_payment, target);
    empty_payment
        .apply_scenario_command(0, &select_branch())
        .expect("choose the optional branch before declining its card payment");
    let payment = empty_payment.state.pending_resolution.as_ref().unwrap();
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::GraveyardCards);
    assert_eq!(payment.presentation.candidates, vec![graveyard]);
    assert_eq!((payment.presentation.min, payment.presentation.max), (0, 1));
    empty_payment
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("an empty min-zero payment declines the graveyard cost");
    assert_eq!(
        empty_payment.state.objects[&graveyard].zone,
        Zone::Graveyard
    );
    assert_eq!(empty_payment.state.players[1].life, 20);
    assert!(empty_payment.state.pending_resolution.is_none());
    assert!(empty_payment.state.stack.is_empty());

    let (mut empty, target, _) = setup(242_003, false);
    let slot = hand_index_for_card(&empty, 0, "heated_argument");
    empty
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    resolve_top(&mut empty);
    assert!(empty.state.pending_resolution.is_none());
    assert_eq!(empty.state.objects[&target].damage, 6);
    assert_eq!(empty.state.players[1].life, 20);
}

#[test]
fn issue_242_sparse_wrong_actor_is_rejected_and_payer_concession_drops_payment() {
    let (mut engine, target, graveyard) = setup_sparse(242_025, true);
    let graveyard = graveyard.unwrap();
    let opposing_graveyard = inject_graveyard_card(&mut engine, 1, "forest");
    cast_and_park_sparse(&mut engine, target);
    engine
        .apply_scenario_command(10, &select_branch())
        .expect("the original spell controller chooses its paid branch");
    let payment = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(payment.deciding_player, 10);
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::GraveyardCards);
    assert_eq!(payment.presentation.candidates, vec![graveyard]);
    assert!(!payment
        .presentation
        .candidates
        .contains(&opposing_graveyard));

    let before = serde_json::to_value(&engine.state).unwrap();
    let command_index = engine.state.command_index;
    let seed = engine.state.seed;
    assert!(engine
        .apply_scenario_command(20, &submit_resolution_choice(vec![graveyard]))
        .is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.seed, seed);

    let departure = engine
        .apply_scenario_command(10, &concede())
        .expect("the payer can concede with the graveyard payment parked");
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[2].life, 20);
    assert_eq!(engine.state.objects[&target].damage, 6);
    assert!(!departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == ChoiceKind::GraveyardCards as i32
                && choice.deciding_player_id != 10
    )));
    assert!(!departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::LifeChanged(change)) if change.delta == -2
    )));
}

fn cast_and_park_sparse(engine: &mut GameEngine, target: u32) -> RuledEventBatch {
    let slot = hand_index_for_card(engine, 0, "heated_argument");
    engine
        .apply_scenario_command(10, &cast_spell(slot, target_object(target)))
        .expect("P10 casts Heated Argument");
    let batch = resolve_top(engine);
    assert_eq!(engine.state.objects[&target].damage, 6);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 10);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    batch
}

#[test]
fn issue_242_damage_replacement_resumes_exact_paid_branch_and_rider_once() {
    use tricerules_core::state::{
        ActiveDamagePrevention, DamagePreventionAmount, DamagePreventionScope,
        ReplacementSourcePresentation,
    };

    let (mut engine, target, graveyard) = setup(242_026, true);
    let graveyard = graveyard.unwrap();
    let first_effect_id = engine.state.next_damage_prevention_effect_id;
    engine.state.damage_prevention_effects = [first_effect_id, first_effect_id + 1]
        .into_iter()
        .map(|id| ActiveDamagePrevention {
            id,
            static_origin: None,
            source_id: None,
            source_label: format!("test prevention {id}"),
            source_presentation: ReplacementSourcePresentation::default(),
            scope: DamagePreventionScope::Recipient(target),
            amount: DamagePreventionAmount::All,
            duration: tricerules_card_model::primitives::EffectDuration::UntilEndOfTurn,
            additional_effect: None,
        })
        .collect();
    engine.state.next_damage_prevention_effect_id = first_effect_id + 2;
    let spell_slot = hand_index_for_card(&engine, 0, "heated_argument");
    let spell = engine.state.players[0].hand[spell_slot];
    let cast = engine
        .apply_scenario_command(0, &cast_spell(spell_slot, target_object(target)))
        .expect("cast Heated Argument");
    let resolution = resolve_top(&mut engine);
    assert_eq!(choice_count(&resolution, ChoiceKind::ReplacementEffect), 1);
    assert_eq!(choice_count(&resolution, ChoiceKind::ResolutionBranch), 0);
    let replacement = resolution
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(required))
                if required.choice_kind == ChoiceKind::ReplacementEffect as i32 =>
            {
                Some(required)
            }
            _ => None,
        })
        .expect("initial damage waits on its replacement-effect choice");
    assert_eq!(replacement.deciding_player_id, 1);
    assert_eq!(engine.state.objects[&target].damage, 0);
    let replacement_id = replacement.replacement_options[0].application_id;
    let resumed = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![replacement_id]))
        .expect("choose one prevention and resume the same Program");
    assert_eq!(choice_count(&resumed, ChoiceKind::ReplacementEffect), 0);
    assert_eq!(choice_count(&resumed, ChoiceKind::ResolutionBranch), 1);
    assert_eq!(engine.state.objects[&target].damage, 0);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );

    let branch = engine
        .apply_scenario_command(0, &select_branch())
        .expect("the resumed Program offers its optional exile branch once");
    assert_eq!(choice_count(&branch, ChoiceKind::GraveyardCards), 1);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![graveyard]
    );
    let before_life = engine.state.players[1].life;
    let payment = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![graveyard]))
        .expect("exile the exact card and finish the rider");
    assert_eq!(engine.state.objects[&graveyard].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&target].damage, 0);
    assert_eq!(engine.state.players[1].life, before_life - 2);
    assert_eq!(choice_count(&payment, ChoiceKind::GraveyardCards), 0);
    assert_eq!(
        payment
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::LifeChanged(change)) if change.delta == -2
            ))
            .count(),
        1,
        "the rider deals damage exactly once after the replacement resumes"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(cast.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackPushed(pushed)) if pushed.object_id == spell
    )));
}

#[test]
fn issue_242_commander_graveyard_cost_exiles_before_rider_and_post_resolution_sba() {
    let (mut engine, target, _) = setup(242_027, false);
    let commander = inject_graveyard_card(&mut engine, 0, "captain_storm,_cosmium_raider");
    engine.state.players[0]
        .declared_commander_object_ids
        .push(commander);
    let initial_commander_generation = engine
        .state
        .zone_change_generation
        .get(&commander)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .commander_sba_checked_generations
        .insert(commander, initial_commander_generation);
    // The scenario starts after the owner has already declined the Commander-in-graveyard SBA.
    let slot = hand_index_for_card(&engine, 0, "heated_argument");
    let spell = engine.state.players[0].hand[slot];
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Heated Argument with a declared Commander in the graveyard");
    resolve_top(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch())
        .expect("choose the optional graveyard-exile branch");
    let payment = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::GraveyardCards);
    assert_eq!(payment.presentation.candidates, vec![commander]);
    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander]))
        .expect("exile the declared Commander as the exact branch cost");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
    assert!(engine.state.zone_change_generation[&commander] > initial_commander_generation);
    assert_eq!(engine.state.players[1].life, 18);
    assert!(
        engine.state.stack.is_empty(),
        "the instant finishes resolving first"
    );
    let commander_prompt = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        commander_prompt.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(commander_prompt.deciding_player, 0);

    let commander_move_index = completion
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(Ev::PermanentMoved(moved))
                    if moved.object_id == commander
                        && moved.destination == Destination::Exile as i32
            )
        })
        .expect("the cost exile is recorded");
    let rider_index = completion
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(Ev::LifeChanged(change)) if change.delta == -2
            )
        })
        .expect("the rider resolves after paying the cost");
    let spell_completion_index = completion
        .events
        .iter()
        .position(|event| matches!(
            &event.ev,
            Some(Ev::StackResolved(resolved))
                if resolved.object_id == spell
                    && resolved.destination
                        == tricerules_proto::ruled::v1::StackResolveDestination::Graveyard as i32
        ))
        .expect("the instant completes resolution");
    let sba_index = completion
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(Ev::ResolutionChoiceRequired(choice))
                    if choice.choice_kind == ChoiceKind::ResolutionBranch as i32
                        && choice.deciding_player_id == 0
            )
        })
        .expect("CR 903.9a is checked after the spell finishes");
    assert!(commander_move_index < rider_index);
    assert!(rider_index < spell_completion_index);
    assert!(spell_completion_index < sba_index);

    let sba_prompt_id = commander_prompt.resolution_prompt_id.get();
    let mut leave_in_exile = select_branch();
    let Some(Cmd::SubmitResolutionChoice(answer)) = leave_in_exile.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = sba_prompt_id;
    answer.selected_branch_index = 1;
    engine
        .apply_scenario_command(0, &leave_in_exile)
        .expect("choose to leave the Commander in exile after resolution");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
}

#[test]
fn issue_242_invalid_graveyard_selections_are_atomic() {
    for case in 0..4 {
        let (mut engine, target, graveyard) = setup(242_010 + case, true);
        let graveyard = graveyard.unwrap();
        let opposing = inject_graveyard_card(&mut engine, 1, "forest");
        cast_and_park(&mut engine, target);
        engine.apply_scenario_command(0, &select_branch()).unwrap();
        let chosen = match case {
            0 => vec![opposing],
            1 => vec![graveyard, graveyard],
            2 => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(graveyard)
                    .or_default() += 1;
                vec![graveyard]
            }
            3 => {
                engine.state.objects.get_mut(&graveyard).unwrap().zone = Zone::Hand;
                engine.state.players[0]
                    .graveyard
                    .retain(|oid| *oid != graveyard);
                engine.state.players[0].hand.push(graveyard);
                vec![graveyard]
            }
            _ => unreachable!(),
        };
        let before = serde_json::to_value(&engine.state).unwrap();
        assert!(engine
            .apply_scenario_command(0, &submit_resolution_choice(chosen))
            .is_err());
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
        assert_eq!(engine.state.players[1].life, 20);
    }
}

#[test]
fn issue_242_illegal_sole_target_fizzles_before_damage_or_payment() {
    let (mut engine, target, graveyard) = setup(242_020, true);
    let slot = hand_index_for_card(&engine, 0, "heated_argument");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != target);
    engine.state.players[1].graveyard.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_top(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(
        engine.state.objects[&graveyard.unwrap()].zone,
        Zone::Graveyard
    );
}

#[test]
fn issue_242_accepted_commands_replay_identically() {
    fn run() -> (Vec<RuledEventBatch>, serde_json::Value) {
        let (mut engine, target, graveyard) = setup(242_030, true);
        let graveyard = graveyard.unwrap();
        let slot = hand_index_for_card(&engine, 0, "heated_argument");
        let cast = engine
            .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap();
        let resolve = resolve_top(&mut engine);
        let branch = engine.apply_scenario_command(0, &select_branch()).unwrap();
        let payment = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![graveyard]))
            .unwrap();
        (
            vec![cast, resolve, branch, payment],
            serde_json::to_value(&engine.state).unwrap(),
        )
    }

    assert_eq!(run(), run());
}
