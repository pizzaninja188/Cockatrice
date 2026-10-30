use super::*;
use tricerules_card_model::primitives::{CardTypeFilter, CounterKind, ZoneCardFilter};

fn setup() -> GameEngine {
    let deck = ["forest", "grizzly_bears"]
        .into_iter()
        .cycle()
        .take(20)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    GameEngine::new(
        tricerules_cards::registry::global(),
        7_130_021,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player hand-entry fixture")
}

fn hand_entry_stack_item(
    engine: &GameEngine,
    up_to_count: u32,
    filter: Option<ZoneCardFilter>,
) -> StackItem {
    let mut ability = engine
        .registry
        .get("kodama_of_the_east_tree")
        .expect("Kodama fixture definition")
        .primary_face()
        .triggered_abilities
        .first()
        .expect("Kodama trigger fixture")
        .clone();
    ability.effect = vec![
        SpellEffectKind::PutPermanentFromHand {
            filter,
            max_mana_value: None,
            up_to_count,
        },
        SpellEffectKind::GainLife {
            amount: tricerules_card_model::Amount::Fixed(2),
            who: tricerules_card_model::primitives::PlayerRecipient::Controller,
        },
    ];
    StackItem {
        prototyped: false,
        mana_colors_spent_to_cast: Default::default(),
        id: u32::MAX,
        controller: 0,
        card_id: "kodama_of_the_east_tree".into(),
        targets: Vec::new(),
        ability_text: Some("hand-entry test".into()),
        source_permanent_id: None,
        source_owner: None,
        source_zone_change: 0,
        trigger_use_key: None,
        source_face_change: 0,
        ability_index: Some(0),
        activated_ability: None,
        activated_ability_occurrence: None,
        triggered_ability: Some(ability),
        is_triggered: true,
        is_copy: false,
        face_index: 0,
        cast_method: SpellCastMethod::Normal,
        returned_attacker_assignment: None,
        chosen_x: 0,
        chosen_modes: Vec::new(),
        cast_cost_receipts: Vec::new(),
        cast_condition_results: Vec::new(),
        cast_occurrence: None,
        cast_by: None,
        payment_result: Default::default(),
        exiled_cohorts: Default::default(),
        chaos_warp_owner_instructions: Default::default(),
        resolution_branch_choices: Default::default(),
        blight_receipts: Vec::new(),
        trigger_context: Default::default(),
    }
}

fn request_hand_entry_choice(
    engine: &mut GameEngine,
    up_to_count: u32,
    filter: Option<ZoneCardFilter>,
) -> (ResolutionProgress, Vec<rv1::RuledEvent>) {
    let top = hand_entry_stack_item(engine, up_to_count, filter.clone());
    request_effect_list(engine, &top)
}

fn request_effect_list(
    engine: &mut GameEngine,
    top: &StackItem,
) -> (ResolutionProgress, Vec<rv1::RuledEvent>) {
    let (effects, spell_label) = engine.build_resolution_effects(top);
    let mut events = Vec::new();
    let progress = engine
        .run_effect_list(top, &spell_label, effects, 0, &mut events)
        .expect("valid hand-entry effect list resolves to its choice");
    (progress, events)
}

fn required_choice(events: &[rv1::RuledEvent]) -> &rv1::ResolutionChoiceRequired {
    events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .expect("hand-entry choice is emitted")
}

fn hand_entry_answer(chosen_object_ids: Vec<u32>) -> rv1::SubmitResolutionChoice {
    rv1::SubmitResolutionChoice {
        chosen_object_ids,
        ..Default::default()
    }
}

fn relevant_objects(engine: &GameEngine, ids: &[u32]) -> Vec<(u32, Zone, i32, String, u64)> {
    ids.iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|object_id| {
            let object = &engine.state.objects[&object_id];
            (
                object_id,
                object.zone,
                object.owner,
                object.card_id.clone(),
                engine
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect()
}

fn reject_then_accept_untouched_companion(
    engine: &mut GameEngine,
    rejected_cohort: &[u32],
    valid_companion: u32,
    expected_reason: &str,
) {
    let hand_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.clone())
        .collect();
    let mut relevant_ids = rejected_cohort.to_vec();
    relevant_ids.push(valid_companion);
    let objects_before = relevant_objects(engine, &relevant_ids);
    let life_before = engine.state.players[0].life;
    let pending_before = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("hand-entry choice remains pending")
        .clone();
    let resume_index_before = pending_before
        .continuation
        .stack()
        .map(|stack| stack.resume_effect_index);
    let previous_result_before = pending_before
        .continuation
        .stack()
        .map(|stack| format!("{:?}", stack.previous_result));

    let error = engine
        .submit_resolution_choice(0, &hand_entry_answer(rejected_cohort.to_vec()))
        .expect_err("the invalid cohort is rejected as one answer");
    match error {
        EngineError::Illegal(reason) => assert_eq!(reason, expected_reason),
        other => panic!("unexpected invalid-cohort error: {other:?}"),
    }

    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.hand.clone())
            .collect::<Vec<_>>(),
        hand_before,
        "a rejected cohort cannot move any member out of its owner's hand"
    );
    assert_eq!(relevant_objects(engine, &relevant_ids), objects_before);
    assert_eq!(engine.state.players[0].life, life_before);
    assert!(engine.state.pending_replacement_event.is_none());
    let pending_after = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("rejected choice keeps its offer parked");
    assert_eq!(
        pending_after.presentation.candidates,
        pending_before.presentation.candidates
    );
    assert_eq!(
        pending_after.presentation.min,
        pending_before.presentation.min
    );
    assert_eq!(
        pending_after.presentation.max,
        pending_before.presentation.max
    );
    assert_eq!(
        pending_after
            .continuation
            .stack()
            .map(|stack| stack.resume_effect_index),
        resume_index_before,
        "rejection preserves the parked effect cursor"
    );
    assert_eq!(
        pending_after
            .continuation
            .stack()
            .map(|stack| format!("{:?}", stack.previous_result)),
        previous_result_before,
        "rejection preserves the previous effect result"
    );

    engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![valid_companion]))
        .expect("the otherwise-valid companion is still selectable");
    assert_eq!(
        engine.state.objects[&valid_companion].zone,
        Zone::Battlefield,
        "the valid answer resumes through the entry batch"
    );
    assert_eq!(engine.state.players[0].life, life_before + 2);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
}

fn set_hand_card_identity(engine: &mut GameEngine, object_id: u32, card_id: &str) {
    let object = engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("fixture card object");
    object.card_id = card_id.into();
    object.copiable_values = None;
    object.token_origin = None;
    object.face_up_index = 0;
    object.face_down = false;
    object.power = None;
    object.toughness = None;
}

fn put_hand_card_on_battlefield(engine: &mut GameEngine, object_id: u32, card_id: &str) {
    let player_index = engine.state.player_idx(0).expect("fixture controller");
    assert!(engine.state.players[player_index].hand.contains(&object_id));
    engine.state.players[player_index]
        .hand
        .retain(|candidate| *candidate != object_id);
    {
        let object = engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("fixture card object");
        object.card_id = card_id.into();
        object.owner = 0;
        object.base_controller = 0;
        object.controller = 0;
        object.zone = Zone::Battlefield;
        object.copiable_values = None;
        object.token_origin = None;
        object.face_up_index = 0;
        object.face_down = false;
        object.attached_to = None;
        object.power = None;
        object.toughness = None;
    }
    engine.state.players[player_index]
        .battlefield
        .push(object_id);
    let generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(object_id, generation + 1);
    engine.emit_static_abilities_on_enter(object_id);
}

fn finish_entry_timestamp_order(engine: &mut GameEngine) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("simultaneous entrants require timestamp ordering");
    assert_eq!(
        pending.presentation.choice_kind,
        rv1::ChoiceKind::SimultaneousEntryOrder
    );
    let order = pending.presentation.candidates.clone();
    engine
        .submit_resolution_choice(0, &hand_entry_answer(order))
        .expect("timestamp ordering resumes the shared entry batch");
}

#[test]
fn hand_entry_choice_uses_authored_fixed_bounds_and_private_selectability() {
    for shape in ["empty", "no_eligible", "short", "full"] {
        let mut engine = setup();
        match shape {
            "empty" => engine.state.players[0].hand.clear(),
            "no_eligible" => {}
            "short" => engine.state.players[0].hand.truncate(2),
            "full" => assert_eq!(engine.state.players[0].hand.len(), 7),
            _ => unreachable!(),
        }
        let hand = engine.state.players[0].hand.clone();
        let filter = (shape == "no_eligible").then_some(ZoneCardFilter {
            card_type: Some(CardTypeFilter::Artifact),
            ..Default::default()
        });

        for up_to_count in 1..=7 {
            engine.state.pending_resolution = None;
            let (progress, events) =
                request_hand_entry_choice(&mut engine, up_to_count, filter.clone());
            assert_eq!(progress, ResolutionProgress::Parked, "shape={shape}");
            let choice = required_choice(&events);
            assert_eq!(choice.min, 0, "shape={shape}, maximum={up_to_count}");
            assert_eq!(choice.max, up_to_count, "shape={shape}");
            assert_eq!(choice.deciding_player_id, 0);
            assert_eq!(choice.choice_kind, rv1::ChoiceKind::HandCards as i32);
            assert_eq!(choice.candidate_object_ids, hand, "shape={shape}");
            assert_eq!(choice.candidate_card_ids.len(), hand.len(), "shape={shape}");
            assert_eq!(choice.candidate_names.len(), hand.len(), "shape={shape}");
            assert_eq!(
                choice.candidate_selectable,
                vec![shape != "no_eligible"; hand.len()],
                "private selectable mask shape={shape}"
            );
            if up_to_count == 1 {
                assert!(choice.prompt_text.contains("a permanent card"));
            } else {
                assert!(
                    choice
                        .prompt_text
                        .contains(&format!("up to {up_to_count} permanent cards")),
                    "authored maximum must appear in the prompt: {}",
                    choice.prompt_text
                );
            }
        }
    }
}

#[test]
fn two_selected_hand_permanents_enter_together_before_resuming_the_effect_tail() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    assert!(hand.len() >= 2);
    let chosen = hand[..2].to_vec();
    let (progress, events) = request_hand_entry_choice(&mut engine, 7, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert_eq!(required_choice(&events).max, 7);

    let result = engine.submit_resolution_choice(
        0,
        &rv1::SubmitResolutionChoice {
            chosen_object_ids: chosen.clone(),
            ..Default::default()
        },
    );
    assert!(
        result.is_ok(),
        "a valid two-card cohort must enter through one shared batch: {result:?}"
    );
    let order_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("simultaneous entries require one timestamp order choice");
    assert_eq!(
        order_choice.presentation.choice_kind,
        rv1::ChoiceKind::SimultaneousEntryOrder
    );
    let timestamp_order = order_choice.presentation.candidates.clone();
    assert_eq!(timestamp_order.len(), chosen.len());
    engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                chosen_object_ids: timestamp_order,
                ..Default::default()
            },
        )
        .expect("the shared entry batch resumes after timestamp ordering");

    for object_id in &chosen {
        assert_eq!(engine.state.objects[object_id].zone, Zone::Battlefield);
    }
    for object_id in hand.iter().filter(|object_id| !chosen.contains(object_id)) {
        assert_eq!(engine.state.objects[object_id].zone, Zone::Hand);
    }
    assert_eq!(engine.state.players[0].life, 22, "effect tail resumes once");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn hand_entry_declining_moves_nothing_and_resumes_the_effect_tail_once() {
    let mut engine = setup();
    let hand_before = engine.state.players[0].hand.clone();
    let generations_before = relevant_objects(&engine, &hand_before);
    let (progress, events) = request_hand_entry_choice(&mut engine, 7, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert_eq!(required_choice(&events).min, 0);

    let result = engine
        .submit_resolution_choice(0, &hand_entry_answer(Vec::new()))
        .expect("zero is a legal optional cohort");

    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(relevant_objects(&engine, &hand_before), generations_before);
    assert_eq!(engine.state.players[0].life, 22, "the tail completes once");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert_eq!(
        result
            .events
            .iter()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::PermanentMoved(_))
            ))
            .count(),
        0,
        "declining creates no entry event"
    );
}

#[test]
fn hand_entry_accepts_all_seven_candidates_as_one_batch() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    assert_eq!(hand.len(), 7);
    let (progress, events) = request_hand_entry_choice(&mut engine, 7, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert_eq!(required_choice(&events).max, 7);

    engine
        .submit_resolution_choice(0, &hand_entry_answer(hand.clone()))
        .expect("the authored seven-card maximum is legal");
    let order = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("all seven entrants are parked for timestamp ordering");
    assert_eq!(
        order.presentation.choice_kind,
        rv1::ChoiceKind::SimultaneousEntryOrder
    );
    let timestamp_order = order.presentation.candidates.clone();
    assert_eq!(timestamp_order.len(), 7);
    engine
        .submit_resolution_choice(0, &hand_entry_answer(timestamp_order))
        .expect("the seven-object entry cohort commits together");

    for object_id in hand {
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Battlefield);
    }
    assert_eq!(
        engine.state.players[0].life, 22,
        "the effect tail completes once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
}

#[test]
fn permanent_cards_drawn_before_the_choice_are_eligible_for_the_same_hand_effect() {
    let mut engine = setup();
    let old_hand = engine.state.players[0].hand.clone();
    let mut top = hand_entry_stack_item(&engine, 7, None);
    top.triggered_ability.as_mut().unwrap().effect = vec![
        SpellEffectKind::GainLife {
            amount: tricerules_card_model::Amount::Fixed(7),
            who: tricerules_card_model::primitives::PlayerRecipient::Controller,
        },
        SpellEffectKind::Draw {
            who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            count: tricerules_card_model::Amount::Fixed(7),
        },
        SpellEffectKind::PutPermanentFromHand {
            filter: None,
            max_mana_value: None,
            up_to_count: 7,
        },
    ];

    let (progress, events) = request_effect_list(&mut engine, &top);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert_eq!(engine.state.players[0].life, 27);
    let hand_after_draw = engine.state.players[0].hand.clone();
    let drawn = hand_after_draw
        .iter()
        .copied()
        .filter(|object_id| !old_hand.contains(object_id))
        .collect::<Vec<_>>();
    assert_eq!(drawn.len(), 7);
    let choice = required_choice(&events);
    assert_eq!(choice.max, 7);
    assert_eq!(choice.candidate_object_ids, hand_after_draw);

    engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![drawn[0]]))
        .expect("a card drawn earlier in this resolving effect is a current hand candidate");

    assert_eq!(engine.state.objects[&drawn[0]].zone, Zone::Battlefield);
    assert!(old_hand
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Hand));
    assert!(drawn[1..]
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Hand));
    assert_eq!(engine.state.players[0].life, 27);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn aura_is_eligible_but_stays_in_hand_when_only_a_coentrant_could_be_its_recipient() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    let aura = hand[0];
    let coentrant = hand[1];
    set_hand_card_identity(&mut engine, aura, "pacifism");
    set_hand_card_identity(&mut engine, coentrant, "grizzly_bears");
    let aura_generation = engine
        .state
        .zone_change_generation
        .get(&aura)
        .copied()
        .unwrap_or(0);
    let (progress, events) = request_hand_entry_choice(&mut engine, 7, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert!(required_choice(&events)
        .candidate_object_ids
        .contains(&aura));
    assert!(required_choice(&events)
        .candidate_selectable
        .iter()
        .zip(&required_choice(&events).candidate_object_ids)
        .any(|(selectable, object_id)| *selectable && *object_id == aura));

    let selection = engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![coentrant, aura]))
        .expect("the coentrant enters while the Aura has no current legal recipient");
    assert!(
        !selection.events.iter().any(|event| matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == rv1::ChoiceKind::AuraPermanent as i32
        )),
        "no coentrant is offered as the Aura's recipient"
    );

    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Hand);
    let expected_hand = hand
        .iter()
        .copied()
        .filter(|object_id| *object_id != coentrant)
        .collect::<Vec<_>>();
    assert_eq!(engine.state.players[0].hand, expected_hand);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&aura)
            .copied()
            .unwrap_or(0),
        aura_generation,
        "skipping the unattached Aura does not create a new object"
    );
    assert_eq!(engine.state.players[0].life, 22, "the tail resumes once");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
}

#[test]
fn aura_can_choose_an_existing_recipient_while_a_coentrant_is_not_yet_on_battlefield() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    let aura = hand[0];
    let coentrant = hand[1];
    let existing_recipient = hand[2];
    put_hand_card_on_battlefield(&mut engine, existing_recipient, "grizzly_bears");
    set_hand_card_identity(&mut engine, aura, "pacifism");
    set_hand_card_identity(&mut engine, coentrant, "grizzly_bears");
    let aura_generation = engine
        .state
        .zone_change_generation
        .get(&aura)
        .copied()
        .unwrap_or(0);
    let coentrant_generation = engine
        .state
        .zone_change_generation
        .get(&coentrant)
        .copied()
        .unwrap_or(0);

    let (progress, events) = request_hand_entry_choice(&mut engine, 7, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert!(required_choice(&events)
        .candidate_object_ids
        .contains(&aura));
    engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![coentrant, aura]))
        .expect("the Aura's as-enters recipient choice parks the full hand cohort");

    let aura_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Aura recipient must be chosen before commit");
    assert_eq!(
        aura_choice.presentation.choice_kind,
        rv1::ChoiceKind::AuraPermanent
    );
    assert_eq!(aura_choice.presentation.candidates, [existing_recipient]);
    assert!(!aura_choice.presentation.candidates.contains(&coentrant));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Hand);
    assert_eq!(
        aura_choice
            .continuation
            .stack()
            .expect("parked hand-entry stack")
            .resume_effect_index,
        Some(1)
    );

    engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![existing_recipient]))
        .expect("an established battlefield recipient remains legal");
    finish_entry_timestamp_order(&mut engine);

    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(crate::state::AttachmentRecipient::Object(
            existing_recipient
        ))
    );
    assert_eq!(
        engine.state.zone_change_generation[&aura],
        aura_generation + 1
    );
    assert_eq!(
        engine.state.zone_change_generation[&coentrant],
        coentrant_generation + 1
    );
    assert_eq!(
        engine.state.players[0].life, 22,
        "the effect tail resumes once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
}

#[test]
fn clone_cannot_copy_a_same_cohort_creature_before_the_cohort_commits() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    let coentrant = hand[0];
    let clone = hand[1];
    set_hand_card_identity(&mut engine, coentrant, "grizzly_bears");
    set_hand_card_identity(&mut engine, clone, "clone");
    let (progress, events) = request_hand_entry_choice(&mut engine, 7, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert!(required_choice(&events)
        .candidate_object_ids
        .contains(&clone));

    let selection = engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![coentrant, clone]))
        .expect("the copy replacement sees the precommit battlefield");
    assert!(selection.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == rv1::ChoiceKind::SimultaneousEntryOrder as i32
    )));
    assert!(!selection.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == rv1::ChoiceKind::CopySource as i32
    )));
    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&clone].zone, Zone::Hand);
    finish_entry_timestamp_order(&mut engine);

    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&clone].zone,
        Zone::Graveyard,
        "the noncopied 0/0 Clone is put into its owner's graveyard by state-based actions"
    );
    assert!(engine.state.objects[&clone].copiable_values.is_none());
    assert_eq!(
        engine.state.players[0].life, 22,
        "the effect tail resumes once"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn clone_copy_offer_excludes_its_same_cohort_creature() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    let coentrant = hand[0];
    let clone = hand[1];
    let existing_creature = hand[2];
    put_hand_card_on_battlefield(&mut engine, existing_creature, "grizzly_bears");
    set_hand_card_identity(&mut engine, coentrant, "grizzly_bears");
    set_hand_card_identity(&mut engine, clone, "clone");

    let (progress, events) = request_hand_entry_choice(&mut engine, 7, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert_eq!(required_choice(&events).max, 7);
    let selection = engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![coentrant, clone]))
        .expect("the selected cohort reaches Clone's as-enters choice");

    let copy_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Clone has a legal preexisting battlefield source")
        .clone();
    assert_eq!(
        copy_choice.presentation.choice_kind,
        rv1::ChoiceKind::CopySource
    );
    assert_eq!(copy_choice.presentation.candidates, [existing_creature]);
    assert!(!copy_choice.presentation.candidates.contains(&coentrant));
    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&clone].zone, Zone::Hand);
    let copy_offer = required_choice(&selection.events);
    assert!(!copy_offer.candidate_object_ids.contains(&coentrant));

    let result = engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![existing_creature]))
        .expect("Clone copies the established creature rather than declining");
    assert!(result.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == rv1::ChoiceKind::SimultaneousEntryOrder as i32
    )));
    finish_entry_timestamp_order(&mut engine);

    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&clone].zone, Zone::Battlefield);
    assert!(engine.state.objects[&clone].copiable_values.is_some());
    assert_eq!(engine.effective_toughness(clone), Some(2));
    assert_eq!(
        engine.state.players[0].life, 22,
        "the effect tail resumes once"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn hand_entry_uses_the_shared_counter_prevention_planner_for_the_whole_batch() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    let entrant = hand[0];
    let coentrant = hand[1];
    set_hand_card_identity(&mut engine, entrant, "brambleback_brute");
    set_hand_card_identity(&mut engine, coentrant, "grizzly_bears");

    // Compose the existing Tatterkite prevention with Brambleback Brute's printed entry
    // counters to exercise the shared planner's recipient-prohibition path in this cohort.
    let mut values = engine
        .copiable_values_for(entrant)
        .expect("Brambleback Brute has copiable values");
    let mut prohibition = engine
        .registry
        .get("tatterkite")
        .expect("Tatterkite definition")
        .primary_face()
        .static_abilities[0]
        .clone();
    prohibition.ability_id = tricerules_card_model::AbilityId::new("fixture_counter_lock")
        .expect("valid fixture ability id");
    values.face.static_abilities.push(prohibition);
    engine
        .state
        .objects
        .get_mut(&entrant)
        .unwrap()
        .copiable_values = Some(values);

    let (progress, events) = request_hand_entry_choice(&mut engine, 2, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert!(required_choice(&events)
        .candidate_object_ids
        .contains(&entrant));
    engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![entrant, coentrant]))
        .expect("both cards use one shared entry batch");
    finish_entry_timestamp_order(&mut engine);

    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&entrant]
            .counters
            .get(&CounterKind::MinusOneMinusOne)
            .copied()
            .unwrap_or(0),
        0,
        "the entrant's current self-prohibition prevents its entry counters"
    );
    assert_eq!(
        engine.state.players[0].life, 22,
        "the effect tail resumes once"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn entry_counter_replacement_pause_preserves_the_hand_cohort_and_tail() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    let laezel = hand[0];
    let season = hand[1];
    let entrant = hand[2];
    let coentrant = hand[3];
    put_hand_card_on_battlefield(&mut engine, laezel, "laezel,_vlaakiths_champion");
    put_hand_card_on_battlefield(&mut engine, season, "doubling_season");
    set_hand_card_identity(&mut engine, entrant, "dockworker_drone");
    set_hand_card_identity(&mut engine, coentrant, "grizzly_bears");
    let entrant_generation = engine
        .state
        .zone_change_generation
        .get(&entrant)
        .copied()
        .unwrap_or(0);
    let coentrant_generation = engine
        .state
        .zone_change_generation
        .get(&coentrant)
        .copied()
        .unwrap_or(0);

    let (progress, events) = request_hand_entry_choice(&mut engine, 2, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    assert_eq!(required_choice(&events).max, 2);
    engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![entrant, coentrant]))
        .expect("the counter-bearing cards enter as one cohort");
    finish_entry_timestamp_order(&mut engine);

    let replacement = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("competing entry-counter replacements park the complete cohort")
        .clone();
    assert_eq!(
        replacement.presentation.choice_kind,
        rv1::ChoiceKind::ReplacementEffect
    );
    assert!(matches!(
        replacement.continuation,
        ResolutionContinuation::EntryCounterReplacement { .. }
    ));
    assert_eq!(replacement.presentation.candidates.len(), 2);
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Hand);
    assert_eq!(
        replacement
            .continuation
            .stack()
            .expect("entry replacement retains the parked hand effect")
            .resume_effect_index,
        Some(1)
    );

    let mut manual_replacement_choices = 0;
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::ReplacementEffect,
            "the counter planner should only reprompt for its next replacement"
        );
        assert!(matches!(
            pending.continuation,
            ResolutionContinuation::EntryCounterReplacement { .. }
        ));
        let chosen_application = pending.presentation.candidates[0];
        engine
            .submit_resolution_choice(0, &hand_entry_answer(vec![chosen_application]))
            .expect("the chosen entry-counter replacement resumes the batch");
        manual_replacement_choices += 1;
        assert!(
            manual_replacement_choices <= 2,
            "replacement order terminates"
        );
    }
    assert!(manual_replacement_choices >= 1);

    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&entrant]
            .counters
            .get(&CounterKind::PlusOnePlusOne)
            .copied()
            .unwrap_or(0),
        4,
        "Lae'zel and Doubling Season apply through the shared entry-counter planner"
    );
    assert_eq!(
        engine.state.zone_change_generation[&entrant],
        entrant_generation + 1
    );
    assert_eq!(
        engine.state.zone_change_generation[&coentrant],
        coentrant_generation + 1
    );
    assert_eq!(
        engine.state.players[0].life, 22,
        "the parked effect tail resumes once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
}

#[test]
fn entry_triggers_are_recorded_for_each_member_of_the_committed_cohort() {
    let mut engine = setup();
    let hand = engine.state.players[0].hand.clone();
    let soul_warden = hand[0];
    let first = hand[1];
    let second = hand[2];
    put_hand_card_on_battlefield(&mut engine, soul_warden, "soul_warden");
    set_hand_card_identity(&mut engine, first, "grizzly_bears");
    set_hand_card_identity(&mut engine, second, "grizzly_bears");
    let (progress, _events) = request_hand_entry_choice(&mut engine, 2, None);
    assert_eq!(progress, ResolutionProgress::Parked);
    engine
        .submit_resolution_choice(0, &hand_entry_answer(vec![first, second]))
        .expect("both creatures enter as one committed cohort");
    finish_entry_timestamp_order(&mut engine);

    assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&second].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .staged_trigger_groups
            .iter()
            .flat_map(|group| &group.triggers)
            .filter(|trigger| trigger.card_id == "soul_warden")
            .count(),
        2,
        "Soul Warden stages one trigger per creature in the committed batch before priority flush"
    );
    assert_eq!(
        engine.state.players[0].life, 22,
        "the effect tail runs once before triggers"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn hand_entry_rejects_duplicate_overlimit_and_eight_card_answers_atomically() {
    let mut duplicate = setup();
    let hand = duplicate.state.players[0].hand.clone();
    request_hand_entry_choice(&mut duplicate, 7, None);
    reject_then_accept_untouched_companion(
        &mut duplicate,
        &[hand[0], hand[0]],
        hand[0],
        "invalid resolution choice",
    );

    let mut overlimit = setup();
    let hand = overlimit.state.players[0].hand.clone();
    request_hand_entry_choice(&mut overlimit, 1, None);
    reject_then_accept_untouched_companion(
        &mut overlimit,
        &[hand[0], hand[1]],
        hand[0],
        "wrong number of cards chosen",
    );

    let mut eight = setup();
    let hand = eight.state.players[0].hand.clone();
    let opponent_card = eight.state.players[1].hand[0];
    request_hand_entry_choice(&mut eight, 7, None);
    let eight_ids = hand.into_iter().chain([opponent_card]).collect::<Vec<_>>();
    let valid_companion = eight.state.players[0].hand[0];
    reject_then_accept_untouched_companion(
        &mut eight,
        &eight_ids,
        valid_companion,
        "wrong number of cards chosen",
    );
}

#[test]
fn hand_entry_rejects_ineligible_and_opponent_hand_cards_atomically() {
    let mut ineligible = setup();
    let hand = ineligible.state.players[0].hand.clone();
    ineligible.state.objects.get_mut(&hand[0]).unwrap().card_id = "mind_stone".into();
    ineligible.state.objects.get_mut(&hand[1]).unwrap().card_id = "grizzly_bears".into();
    let artifact_filter = Some(ZoneCardFilter {
        card_type: Some(CardTypeFilter::Artifact),
        ..Default::default()
    });
    let (progress, events) = request_hand_entry_choice(&mut ineligible, 7, artifact_filter);
    assert_eq!(progress, ResolutionProgress::Parked);
    let choice = required_choice(&events);
    assert!(choice.candidate_selectable[0]);
    assert!(!choice.candidate_selectable[1]);
    assert!(ineligible
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .contains(&hand[0]));
    assert!(!ineligible
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .contains(&hand[1]));
    reject_then_accept_untouched_companion(
        &mut ineligible,
        &[hand[0], hand[1]],
        hand[0],
        "invalid resolution choice",
    );

    let mut opponent = setup();
    let player_hand = opponent.state.players[0].hand.clone();
    let opponent_card = opponent.state.players[1].hand[0];
    request_hand_entry_choice(&mut opponent, 7, None);
    reject_then_accept_untouched_companion(
        &mut opponent,
        &[player_hand[0], opponent_card],
        player_hand[0],
        "invalid resolution choice",
    );
}

#[test]
fn hand_entry_revalidates_generation_owner_zone_and_permanent_status_atomically() {
    let mut stale_generation = setup();
    let hand = stale_generation.state.players[0].hand.clone();
    request_hand_entry_choice(&mut stale_generation, 7, None);
    *stale_generation
        .state
        .zone_change_generation
        .entry(hand[1])
        .or_insert(0) += 1;
    reject_then_accept_untouched_companion(
        &mut stale_generation,
        &[hand[0], hand[1]],
        hand[0],
        "chosen hand-entry cohort is stale or no longer legal",
    );

    let mut changed_owner = setup();
    let hand = changed_owner.state.players[0].hand.clone();
    request_hand_entry_choice(&mut changed_owner, 7, None);
    changed_owner.state.objects.get_mut(&hand[1]).unwrap().owner = 1;
    reject_then_accept_untouched_companion(
        &mut changed_owner,
        &[hand[0], hand[1]],
        hand[0],
        "chosen hand-entry cohort is stale or no longer legal",
    );

    let mut changed_zone = setup();
    let hand = changed_zone.state.players[0].hand.clone();
    request_hand_entry_choice(&mut changed_zone, 7, None);
    changed_zone.state.players[0]
        .hand
        .retain(|object_id| *object_id != hand[1]);
    changed_zone.state.players[0].graveyard.push(hand[1]);
    changed_zone.state.objects.get_mut(&hand[1]).unwrap().zone = Zone::Graveyard;
    *changed_zone
        .state
        .zone_change_generation
        .entry(hand[1])
        .or_insert(0) += 1;
    reject_then_accept_untouched_companion(
        &mut changed_zone,
        &[hand[0], hand[1]],
        hand[0],
        "chosen hand-entry cohort is stale or no longer legal",
    );

    let mut changed_type = setup();
    let hand = changed_type.state.players[0].hand.clone();
    request_hand_entry_choice(&mut changed_type, 7, None);
    changed_type
        .state
        .objects
        .get_mut(&hand[1])
        .unwrap()
        .card_id = "lightning_bolt".into();
    reject_then_accept_untouched_companion(
        &mut changed_type,
        &[hand[0], hand[1]],
        hand[0],
        "chosen hand-entry cohort is stale or no longer legal",
    );
}
