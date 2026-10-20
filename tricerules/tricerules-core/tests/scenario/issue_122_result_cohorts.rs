use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_core::Zone;

type AcceptedStep = (i32, RuledCommand, RuledEventBatch);

fn prepare_grab_the_prize(discard_card: &str, seed: u64, player_ids: &[i32]) -> GameEngine {
    let decks = Some(
        player_ids
            .iter()
            .enumerate()
            .map(|(index, _)| {
                if index == 0 {
                    deck_with("mountain", &["grab_the_prize", discard_card])
                } else {
                    deck_with("forest", &[])
                }
            })
            .collect(),
    );
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        player_ids,
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "grab_the_prize");
    ensure_card_in_hand(&mut engine, 0, discard_card);
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    engine
}

fn grab_cast_command(engine: &GameEngine, discard_card: &str) -> RuledCommand {
    let spell_slot = hand_index_for_card(engine, 0, "grab_the_prize");
    let discard_slot = hand_index_for_card(engine, 0, discard_card);
    cast_spell_with_costs(
        spell_slot,
        vec![],
        vec![hand_cost_selection(0, discard_slot as u32)],
    )
}

fn cast_grab_the_prize(discard_card: &str, seed: u64) -> GameEngine {
    let mut engine = prepare_grab_the_prize(discard_card, seed, &[0, 1]);
    engine
        .apply_scenario_command(0, &grab_cast_command(&engine, discard_card))
        .expect("Grab the Prize should accept its discard payment");
    engine
}

fn accepted_step(
    engine: &mut GameEngine,
    steps: &mut Vec<AcceptedStep>,
    actor: i32,
    command: RuledCommand,
) -> RuledEventBatch {
    let batch = engine
        .apply_scenario_command(actor, &command)
        .expect("accepted command in Grab scenario");
    steps.push((actor, command, batch.clone()));
    batch
}

fn replay_accepted_steps(engine: &mut GameEngine, steps: &[AcceptedStep]) {
    for (actor, command, expected_batch) in steps {
        let batch = engine
            .apply_scenario_command(*actor, command)
            .expect("recorded accepted command replays");
        assert_eq!(&batch, expected_batch, "replayed event batch");
    }
}

fn branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn record_priority_round(
    engine: &mut GameEngine,
    steps: &mut Vec<AcceptedStep>,
) -> RuledEventBatch {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("consistent priority pass count");
    let mut last = RuledEventBatch::default();
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        last = accepted_step(engine, steps, actor, pass());
    }
    last
}

#[test]
fn grab_the_prize_uses_the_paid_cards_type_snapshot() {
    let owner = tricerules_card_model::ProgramOwnerId::face(
        "grab_the_prize",
        tricerules_card_model::CardFaceId::new("grab_the_prize").unwrap(),
    );
    assert!(
        tricerules_cards::registry::global()
            .compiled_program_for_owner(&owner)
            .is_some(),
        "the actual Grab the Prize spell face resolves through its compiled Program owner"
    );

    let mut nonland = cast_grab_the_prize("grizzly_bears", 12_201);
    let hand_after_payment = nonland.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut nonland);
    assert_eq!(nonland.state.players[0].hand.len(), hand_after_payment + 2);
    assert_eq!(nonland.state.players[1].life, 18);

    let mut land = cast_grab_the_prize("mountain", 12_202);
    let hand_after_payment = land.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut land);
    assert_eq!(land.state.players[0].hand.len(), hand_after_payment + 2);
    assert_eq!(land.state.players[1].life, 20);
}

#[test]
fn grab_the_prize_abundance_draws_resume_once_before_damage_and_replay_identically() {
    fn fixture(seed: u64) -> GameEngine {
        let mut engine = prepare_grab_the_prize("grizzly_bears", seed, &[0, 1]);
        inject_permanent_on_battlefield(&mut engine, 0, "abundance");
        engine
    }

    let mut engine = fixture(12_220);
    let library_before = engine.state.players[0].library.len();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    let mut accepted = Vec::new();

    let cast = grab_cast_command(&engine, "grizzly_bears");
    accepted_step(&mut engine, &mut accepted, 0, cast);
    let hand_after_payment = engine.state.players[0].hand.len();
    let resolution = record_priority_round(&mut engine, &mut accepted);
    let first = find_resolution_choice(&resolution).expect("first Abundance replacement choice");
    assert_eq!(first.deciding_player_id, 0);

    accepted_step(&mut engine, &mut accepted, 0, branch(0));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Abundance asks for land or nonland")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    accepted_step(&mut engine, &mut accepted, 0, branch(0));
    assert_eq!(engine.state.players[0].hand.len(), hand_after_payment + 1);
    assert_eq!(
        engine.state.players[1].life, 20,
        "damage waits for both draws"
    );
    assert!(
        engine.state.pending_resolution.is_some(),
        "second draw is pending"
    );

    accepted_step(&mut engine, &mut accepted, 0, branch(0));
    accepted_step(&mut engine, &mut accepted, 0, branch(0));
    assert_eq!(engine.state.players[0].hand.len(), hand_after_payment + 2);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before,
        "Abundance replaces both draw events"
    );
    assert_eq!(
        engine.state.players[1].life, 18,
        "the resumed damage tail runs once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    let mut replay = fixture(12_220);
    replay_accepted_steps(&mut replay, &accepted);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn grab_the_prize_rejects_a_normal_cast_without_its_mandatory_discard_atomically() {
    let mut engine = prepare_grab_the_prize("grizzly_bears", 12_221, &[0, 1]);
    let spell_slot = hand_index_for_card(&engine, 0, "grab_the_prize");
    let before = engine.diagnostic_snapshot().unwrap();

    let error = engine
        .apply_scenario_command(0, &cast_spell(spell_slot, Vec::new()))
        .expect_err("the mandatory discard cost is required");
    assert!(
        error
            .to_string()
            .contains("missing additional cost selection"),
        "{error}"
    );
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn grab_the_prize_each_opponent_damage_resumes_after_competing_first_player_preventions_once() {
    use tricerules_core::state::{
        ActiveDamagePrevention, DamagePreventionAmount, DamagePreventionScope,
        ReplacementSourcePresentation,
    };

    fn fixture(seed: u64) -> GameEngine {
        let mut engine = prepare_grab_the_prize("grizzly_bears", seed, &[0, 1, 2]);
        let first_effect_id = engine.state.next_damage_prevention_effect_id;
        engine.state.damage_prevention_effects = [first_effect_id, first_effect_id + 1]
            .into_iter()
            .map(|id| ActiveDamagePrevention {
                id,
                static_origin: None,
                source_id: None,
                source_label: format!("test prevention {id}"),
                source_presentation: ReplacementSourcePresentation::default(),
                scope: DamagePreventionScope::Recipient(
                    u32::try_from(engine.state.players[1].id).expect("positive player ID"),
                ),
                amount: DamagePreventionAmount::All,
                duration: tricerules_card_model::primitives::EffectDuration::UntilEndOfTurn,
                additional_effect: None,
            })
            .collect();
        engine.state.next_damage_prevention_effect_id = first_effect_id + 2;
        engine
    }

    let mut engine = fixture(12_222);
    let mut accepted = Vec::new();
    let cast = grab_cast_command(&engine, "grizzly_bears");
    accepted_step(&mut engine, &mut accepted, 0, cast);
    let caster_hand_after_payment = engine.state.players[0].hand.len();
    let caster_library_after_payment = engine.state.players[0].library.len();
    let caster_draws_after_payment = engine.state.turn_history.current.player(0).cards_drawn;
    let resolution = record_priority_round(&mut engine, &mut accepted);
    let replacement = find_resolution_choice(&resolution).expect("first opponent prevention offer");
    assert_eq!(replacement.choice_kind(), ChoiceKind::ReplacementEffect);
    assert_eq!(replacement.deciding_player_id, 1);
    assert_eq!(replacement.replacement_options.len(), 2);
    assert_eq!(
        engine.state.players[0].hand.len(),
        caster_hand_after_payment + 2
    );
    assert_eq!(
        engine.state.players[0].library.len(),
        caster_library_after_payment - 2
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        caster_draws_after_payment + 2
    );
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[2].life, 20);
    let draw_state_at_park = (
        engine.state.players[0].hand.len(),
        engine.state.players[0].library.len(),
        engine.state.turn_history.current.player(0).cards_drawn,
    );

    let application_id = replacement.replacement_options[0].application_id;
    let answer = submit_resolution_choice(vec![application_id]);
    let before_wrong_actor = engine.diagnostic_snapshot().unwrap();
    let wrong_actor = engine
        .apply_scenario_command(2, &answer)
        .expect_err("the prevention choice belongs to the first recipient");
    assert!(!wrong_actor.to_string().is_empty());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_wrong_actor);

    let resumed = accepted_step(&mut engine, &mut accepted, 1, answer);
    assert!(
        find_resolution_choice(&resumed).is_none(),
        "there is no second recipient prevention choice"
    );
    assert_eq!(
        (
            engine.state.players[0].hand.len(),
            engine.state.players[0].library.len(),
            engine.state.turn_history.current.player(0).cards_drawn,
        ),
        draw_state_at_park,
        "resuming later-opponent damage does not repeat Grab's Draw instruction"
    );
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(
        engine.state.players[1].life, 20,
        "the first opponent is prevented"
    );
    assert_eq!(
        engine.state.players[2].life, 18,
        "the later opponent is damaged once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    let mut replay = fixture(12_222);
    replay_accepted_steps(&mut replay, &accepted);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn grab_the_prize_keeps_payment_results_for_copies_and_later_zone_changes() {
    let mut moved = cast_grab_the_prize("grizzly_bears", 12_205);
    let paid = moved.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|object_id| moved.state.objects[object_id].card_id == "grizzly_bears")
        .expect("paid card in graveyard");
    moved.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != paid);
    moved.state.players[0].exile.push(paid);
    moved.state.objects.get_mut(&paid).unwrap().zone = Zone::Exile;
    *moved.state.zone_change_generation.entry(paid).or_default() += 1;
    resolve_entire_stack_two_player(&mut moved);
    assert_eq!(moved.state.players[1].life, 18);

    let mut copied = cast_grab_the_prize("grizzly_bears", 12_206);
    inject_card_into_hand(&mut copied, 1, "twincast");
    give_mana(
        &mut copied,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let original = copied.state.stack.last().unwrap().id;
    copied.apply_scenario_command(0, &pass()).unwrap();
    let twincast = hand_index_for_card(&copied, 1, "twincast");
    copied
        .apply_scenario_command(
            1,
            &cast_spell(
                twincast,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    expected_zone_change_generation: None,
                    object_id: original,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .unwrap();
    resolve_entire_stack_two_player(&mut copied);
    assert_eq!(copied.state.players[0].life, 18);
    assert_eq!(copied.state.players[1].life, 18);
}

#[test]
fn grab_the_prize_copy_keeps_paid_facts_after_original_payer_concedes() {
    fn fixture(seed: u64) -> GameEngine {
        let mut engine = prepare_grab_the_prize("grizzly_bears", seed, &[0, 1, 2]);
        inject_card_into_hand(&mut engine, 1, "twincast");
        give_mana(
            &mut engine,
            1,
            ManaGift {
                u: 2,
                ..Default::default()
            },
        );
        engine
    }

    let mut engine = fixture(12_223);
    let mut accepted = Vec::new();
    let cast = grab_cast_command(&engine, "grizzly_bears");
    accepted_step(&mut engine, &mut accepted, 0, cast);
    accepted_step(&mut engine, &mut accepted, 0, pass());
    let original = engine.state.stack.last().expect("original Grab spell").id;
    let twincast_slot = hand_index_for_card(&engine, 1, "twincast");
    accepted_step(
        &mut engine,
        &mut accepted,
        1,
        cast_spell(
            twincast_slot,
            vec![tricerules_proto::ruled::v1::TargetRef {
                expected_zone_change_generation: None,
                object_id: original,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    );
    let twincast_resolution = record_priority_round(&mut engine, &mut accepted);
    assert!(find_resolution_choice(&twincast_resolution).is_none());

    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy && item.card_id == "grab_the_prize")
        .expect("Twincast created a Grab copy");
    assert_eq!(copy.controller, 1);
    let copy_id = copy.id;

    accepted_step(&mut engine, &mut accepted, 0, concede());
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.stack.iter().any(|item| item.id == copy_id));
    let retained = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == copy_id)
        .expect("copy survives the original caster's concession");
    assert!(retained.is_copy);

    let copy_controller_hand = engine.state.players[1].hand.len();
    let copy_controller_library = engine.state.players[1].library.len();
    let copy_controller_draws = engine.state.turn_history.current.player(1).cards_drawn;

    record_priority_round(&mut engine, &mut accepted);
    assert_eq!(
        engine.state.players[1].hand.len(),
        copy_controller_hand + 2,
        "the copy's Draw instruction resolves once for its controller"
    );
    assert_eq!(
        engine.state.players[1].library.len(),
        copy_controller_library - 2
    );
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        copy_controller_draws + 2
    );
    assert_eq!(
        engine.state.players[1].life, 20,
        "a copy does not damage its controller"
    );
    assert_eq!(
        engine.state.players[2].life, 18,
        "the copy uses its controller's opponents"
    );
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());

    let mut replay = fixture(12_223);
    replay_accepted_steps(&mut replay, &accepted);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

fn cast_gerrards_verdict(cards: &[&str], seed: u64) -> (GameEngine, Vec<u32>) {
    let decks = Some(vec![
        deck_with("plains", &["gerrards_verdict"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let chosen = cards
        .iter()
        .map(|card| inject_card_into_hand(&mut engine, 1, card))
        .collect::<Vec<_>>();
    ensure_card_in_hand(&mut engine, 0, "gerrards_verdict");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            b: 1,
            ..Default::default()
        },
    );
    let spell_slot = hand_index_for_card(&engine, 0, "gerrards_verdict");
    engine
        .apply_scenario_command(0, &cast_spell(spell_slot, target_player(1)))
        .expect("cast Gerrard's Verdict");
    engine.apply_scenario_command(0, &pass()).unwrap();
    let parked = engine.apply_scenario_command(1, &pass()).unwrap();
    let choice = find_resolution_choice(&parked).expect("discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!((choice.min, choice.max), (2, 2));
    (engine, chosen)
}

#[test]
fn gerrards_verdict_counts_the_exact_discarded_land_cohort_after_resume() {
    let (mut one_land, chosen) = cast_gerrards_verdict(&["forest", "grizzly_bears"], 12_203);
    one_land
        .apply_scenario_command(1, &submit_resolution_choice(chosen))
        .expect("submit one land and one nonland");
    assert_eq!(one_land.state.players[0].life, 23);

    let (mut two_lands, chosen) = cast_gerrards_verdict(&["forest", "mountain"], 12_204);
    two_lands
        .apply_scenario_command(1, &submit_resolution_choice(chosen))
        .expect("submit two lands");
    assert_eq!(two_lands.state.players[0].life, 26);
}

#[test]
fn gerrards_verdict_rejects_a_stale_discard_choice_without_losing_the_prompt() {
    let (mut engine, chosen) = cast_gerrards_verdict(&["forest", "mountain"], 12_208);
    *engine
        .state
        .zone_change_generation
        .entry(chosen[0])
        .or_default() += 1;

    let error = engine
        .apply_scenario_command(1, &submit_resolution_choice(chosen))
        .expect_err("stale chosen identity must fail closed");
    assert!(error.to_string().contains("stale"));
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.players[0].life, 20);
}
