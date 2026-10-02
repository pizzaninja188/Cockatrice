use super::helpers::*;
use prost::Message;
use tricerules_cards::primitives::{BasicLandType, ConditionObjectRef, ControllerReference};
use tricerules_cards::{
    CardRegistry, ContinuousEffectKind, CounterKind, EffectDuration, GameCondition,
};
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

fn resolve_until_choice(engine: &mut GameEngine) -> rv1::ResolutionChoiceRequired {
    for _ in 0..20 {
        let actor = engine.state.priority_player_id();
        let batch = engine.apply_command(actor, &pass()).unwrap();
        if let Some(choice) = find_resolution_choice(&batch) {
            return choice.clone();
        }
    }
    panic!("no resolution choice");
}

fn answer_replacements(
    engine: &mut GameEngine,
    mut choice: rv1::ResolutionChoiceRequired,
    prefer: &str,
) -> Option<rv1::ResolutionChoiceRequired> {
    for _ in 0..40 {
        if choice.choice_kind() != rv1::ChoiceKind::ReplacementEffect {
            return Some(choice);
        }
        assert_eq!((choice.min, choice.max), (1, 1));
        let selected = choice
            .replacement_options
            .iter()
            .find(|option| option.source_card_name == prefer)
            .unwrap_or(&choice.replacement_options[0])
            .application_id;
        let batch = engine
            .apply_command(
                choice.deciding_player_id,
                &submit_resolution_choice(vec![selected]),
            )
            .unwrap();
        match find_resolution_choice(&batch) {
            Some(next) => choice = next.clone(),
            None => {
                assert!(engine.state.pending_resolution.is_none());
                return None;
            }
        }
    }
    panic!("replacement recursion");
}

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with("island", &["thought_reflection", "teferis_ageless_insight"]);
    let mut engine = GameEngine::new(
        seed,
        &[4, 9, 27],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("complete selected draw-replacement cards must be admitted");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn paid_spell(engine: &mut GameEngine, card: &str) -> u32 {
    let caster = engine.state.players[0].id;
    let oid = inject_card_into_hand(engine, 0, card);
    give_mana(
        engine,
        caster,
        ManaGift {
            u: 3,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, card);
    engine
        .apply_command(caster, &cast_spell(slot, Vec::new()))
        .unwrap();
    oid
}

fn resolve(engine: &mut GameEngine, oid: u32) {
    for _ in 0..12 {
        if !engine.state.stack.iter().any(|item| item.id == oid) {
            return;
        }
        assert!(
            engine.state.pending_resolution.is_none(),
            "unexpected replacement choice"
        );
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
    panic!("spell did not finish");
}

#[test]
fn brainstorm_stays_physically_on_stack_until_put_back_completes() {
    let mut engine = setup(507_004);
    let spell = paid_spell(&mut engine, "brainstorm");
    let generation = engine.state.zone_change_generation[&spell];
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert_eq!(engine.state.zone_change_generation[&spell], generation);
    let batch = engine
        .apply_command(
            4,
            &submit_resolution_choice(choice.candidate_object_ids[..2].to_vec()),
        )
        .unwrap();
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.zone_change_generation[&spell], generation + 1);
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
                Some(rv1::ruled_event::Ev::StackResolved(exit)) if exit.object_id == spell
            ))
            .count(),
        1
    );
}

#[test]
fn reflection_actual_paid_cast_doubles_each_divination_draw_through_zone_funnel() {
    let mut engine = setup(506_001);
    let reflection = paid_spell(&mut engine, "thought_reflection");
    resolve(&mut engine, reflection);
    assert_eq!(engine.state.objects[&reflection].zone, Zone::Battlefield);
    let spell = paid_spell(&mut engine, "divination");
    let before = engine.state.players[0].hand.len();
    let drawn = engine.state.players[0]
        .library
        .iter()
        .take(4)
        .copied()
        .collect::<Vec<_>>();
    let generations = drawn
        .iter()
        .map(|oid| {
            engine
                .state
                .zone_change_generation
                .get(oid)
                .copied()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    resolve(&mut engine, spell);
    assert_eq!(engine.state.players[0].hand.len(), before + 4);
    for (oid, generation) in drawn.into_iter().zip(generations) {
        assert!(engine.state.players[0].hand.contains(&oid));
        assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
        assert_eq!(engine.state.zone_change_generation[&oid], generation + 1);
        assert_eq!(engine.state.objects[&oid].owner, 4);
    }
}

#[test]
fn two_reflections_child_sets_are_isolated_and_reset_for_each_original_draw() {
    let mut engine = setup(506_002);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    paid_spell(&mut engine, "divination");
    let before = engine.state.players[0].hand.len();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.deciding_player_id, 4);
    assert_eq!(choice.replacement_options.len(), 2);
    assert!(answer_replacements(&mut engine, choice, "Thought Reflection").is_none());
    assert_eq!(engine.state.players[0].hand.len(), before + 8);
}

#[test]
fn insight_actual_paid_cast_upkeep_does_not_consume_first_own_draw_step_exception() {
    let mut engine = setup(506_003);
    let insight = paid_spell(&mut engine, "teferis_ageless_insight");
    resolve(&mut engine, insight);
    engine.state.turn_step = TurnStep::Upkeep;
    engine.state.priority_idx = 0;
    let brainstorm = paid_spell(&mut engine, "brainstorm");
    let before = engine.state.players[0].hand.len();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!(engine.state.players[0].hand.len(), before + 6);
    let ids = choice.candidate_object_ids[..2].to_vec();
    engine
        .apply_command(4, &submit_resolution_choice(ids))
        .unwrap();
    assert_eq!(engine.state.objects[&brainstorm].zone, Zone::Graveyard);
    let before_step = engine.state.players[0].hand.len();
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
    assert_eq!(engine.state.turn_step, TurnStep::Draw);
    assert_eq!(engine.state.players[0].hand.len(), before_step + 1);
    paid_spell(&mut engine, "brainstorm");
    let before_later = engine.state.players[0].hand.len();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!(engine.state.players[0].hand.len(), before_later + 6);
}

#[test]
fn reflection_insight_first_own_step_is_three_in_both_initial_source_orders() {
    for prefer in ["Thought Reflection", "Teferi's Ageless Insight"] {
        let mut engine = setup(506_004);
        for _ in 0..20 {
            inject_library_card(&mut engine, 0, "island");
        }
        inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        inject_permanent_on_battlefield(&mut engine, 0, "teferis_ageless_insight");
        engine.state.turn_step = TurnStep::Upkeep;
        engine.state.priority_idx = 0;
        let before = engine.state.players[0].hand.len();
        for _ in 0..3 {
            let actor = engine.state.priority_player_id();
            engine.apply_command(actor, &pass()).unwrap();
        }
        assert_eq!(engine.state.turn_step, TurnStep::Draw);
        assert_eq!(engine.state.players[0].hand.len(), before + 3);
        paid_spell(&mut engine, "brainstorm");
        let before_later = engine.state.players[0].hand.len();
        let choice = resolve_until_choice(&mut engine);
        assert_eq!(choice.choice_kind(), rv1::ChoiceKind::ReplacementEffect);
        let putback = answer_replacements(&mut engine, choice, prefer).unwrap();
        assert_eq!(putback.choice_kind(), rv1::ChoiceKind::HandCards);
        assert_eq!(engine.state.players[0].hand.len(), before_later + 12);
    }
}

#[test]
fn draw_order_choices_reject_foreign_duplicate_unknown_and_prior_prompt_handles_atomically() {
    let mut engine = setup(506_005);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "teferis_ageless_insight");
    paid_spell(&mut engine, "divination");
    let first = resolve_until_choice(&mut engine);
    let handle = first.replacement_options[0].application_id;
    for (actor, ids) in [
        (9, vec![handle]),
        (4, vec![]),
        (4, vec![u32::MAX]),
        (4, vec![handle, handle]),
    ] {
        let before = serde_json::to_vec(&engine.state).unwrap();
        assert!(engine
            .apply_command(actor, &submit_resolution_choice(ids))
            .is_err());
        assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    }
    let batch = engine
        .apply_command(4, &submit_resolution_choice(vec![handle]))
        .unwrap();
    let next = find_resolution_choice(&batch).unwrap();
    assert!(!next.candidate_object_ids.contains(&handle));
    let before = serde_json::to_vec(&engine.state).unwrap();
    assert!(engine
        .apply_command(4, &submit_resolution_choice(vec![handle]))
        .is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
}

#[test]
fn concession_of_foreign_replacement_source_rebuilds_parked_draw_and_finishes_once() {
    let mut engine = setup(506_006);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    let foreign = inject_permanent_on_battlefield(&mut engine, 1, "thought_reflection");
    engine
        .state
        .objects
        .get_mut(&foreign)
        .unwrap()
        .base_controller = 4;
    engine.state.objects.get_mut(&foreign).unwrap().controller = 4;
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != foreign);
    engine.state.players[0].battlefield.push(foreign);
    paid_spell(&mut engine, "divination");
    let before = engine.state.players[0].hand.len();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.replacement_options.len(), 2);
    engine.apply_command(9, &concede()).unwrap();
    assert!(
        engine.state.pending_resolution.is_none(),
        "departed source must not strand the remaining drawer"
    );
    assert_eq!(engine.state.players[0].hand.len(), before + 4);
}

#[test]
fn brainstorm_expanded_draw_and_private_ordered_putback_each_change_generation_once() {
    let mut engine = setup(506_007);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    paid_spell(&mut engine, "brainstorm");
    let drawn = engine.state.players[0]
        .library
        .iter()
        .take(6)
        .copied()
        .collect::<Vec<_>>();
    let generations = drawn
        .iter()
        .map(|id| {
            engine
                .state
                .zone_change_generation
                .get(id)
                .copied()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let before = engine.state.players[0].hand.len();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max, choice.ordered), (2, 2, true));
    assert!(choice.public_reveal.is_none());
    assert_eq!(choice.deciding_player_id, 4);
    assert_eq!(engine.state.players[0].hand.len(), before + 6);
    engine
        .apply_command(4, &submit_resolution_choice(drawn[..2].to_vec()))
        .unwrap();
    assert_eq!(*engine.state.players[0].library.front().unwrap(), drawn[1]);
    assert_eq!(engine.state.players[0].hand.len(), before + 4);
    for (i, oid) in drawn.iter().enumerate() {
        assert_eq!(
            engine.state.zone_change_generation[oid],
            generations[i] + if i < 2 { 2 } else { 1 }
        );
    }
}

#[test]
fn draw_then_discard_replacements_finish_before_frantic_search_private_hand_and_trailing_land_choice(
) {
    let mut engine = setup(506_008);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "teferis_ageless_insight");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "island");
    engine.state.objects.get_mut(&land).unwrap().tapped = true;
    let spell = paid_spell(&mut engine, "frantic_search");
    let before = engine.state.players[0].hand.len();
    let replacement = resolve_until_choice(&mut engine);
    let discard = answer_replacements(&mut engine, replacement, "Thought Reflection").unwrap();
    assert_eq!(discard.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!(engine.state.players[0].hand.len(), before + 8);
    let batch = engine
        .apply_command(
            4,
            &submit_resolution_choice(discard.candidate_object_ids[..2].to_vec()),
        )
        .unwrap();
    let lands =
        find_resolution_choice(&batch).expect("authored trailing land choice must run once");
    assert!(lands.candidate_object_ids.contains(&land));
    assert_eq!(engine.state.players[0].hand.len(), before + 6);
    engine
        .apply_command(4, &submit_resolution_choice(vec![land]))
        .unwrap();
    assert!(!engine.state.objects[&land].tapped);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn copied_replacement_uses_live_conditional_ability_availability_and_foreign_draw_step() {
    for tapped in [false, true] {
        let mut engine = setup(506_009);
        let copy = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let face = CardRegistry::global()
            .get("thought_reflection")
            .unwrap()
            .primary_face()
            .clone();
        engine.state.objects.get_mut(&copy).unwrap().copiable_values =
            Some(tricerules_core::state::CopiableValues {
                source_card_id: "thought_reflection".into(),
                source_face_index: 0,
                display_name: face.name.clone(),
                face,
                room_faces: None,
            });
        let source = inject_permanent_on_battlefield(&mut engine, 1, "island");
        engine.state.objects.get_mut(&source).unwrap().tapped = tapped;
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: Some(source),
            trigger_grant_origin: None,
            affected: AffectedScope::Single(copy),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: Some(GameCondition::ObjectTapped {
                object: ConditionObjectRef::Source,
                tapped: true,
            }),
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: engine.state.command_index,
        });
        let spell = paid_spell(&mut engine, "divination");
        let before = engine.state.players[0].hand.len();
        resolve(&mut engine, spell);
        assert_eq!(
            engine.state.players[0].hand.len(),
            before + if tapped { 2 } else { 4 }
        );
    }
    let mut engine = setup(506_010);
    inject_permanent_on_battlefield(&mut engine, 0, "teferis_ageless_insight");
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Draw;
    paid_spell(&mut engine, "brainstorm");
    let before = engine.state.players[0].hand.len();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!(engine.state.players[0].hand.len(), before + 6);
}

#[test]
fn apnap_fanout_parks_at_nonconsecutive_drawer_after_prior_partial_draws_and_triggers() {
    let mut engine = setup(506_011);
    let jace = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    for seat in 0..3 {
        inject_permanent_on_battlefield(&mut engine, seat, "thought_reflection");
    }
    inject_permanent_on_battlefield(&mut engine, 1, "thought_reflection");
    let ominous = inject_permanent_on_battlefield(&mut engine, 0, "ominous_seas");
    let before = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.len())
        .collect::<Vec<_>>();
    engine
        .apply_command(4, &activate_ability(jace, 0, Vec::new()))
        .unwrap();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.deciding_player_id, 9);
    assert_eq!(engine.state.players[0].hand.len(), before[0] + 2);
    assert_eq!(engine.state.players[1].hand.len(), before[1]);
    assert_eq!(engine.state.players[2].hand.len(), before[2]);
    assert!(answer_replacements(&mut engine, choice, "Thought Reflection").is_none());
    assert_eq!(engine.state.players[1].hand.len(), before[1] + 4);
    assert_eq!(engine.state.players[2].hand.len(), before[2] + 2);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.source_permanent_id == Some(ominous))
            .count(),
        2
    );
    for _ in 0..20 {
        if engine.state.stack.is_empty() {
            break;
        }
        answer_trigger_order_in_engine_order(&mut engine);
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
    assert_eq!(
        engine.state.objects[&ominous].counter_count(CounterKind::Foreshadow),
        2
    );
}

#[test]
fn mandatory_draw_replacement_rejects_decline_and_unrelated_answer_fields_atomically() {
    let mut engine = setup(506_012);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "teferis_ageless_insight");
    paid_spell(&mut engine, "divination");
    let choice = resolve_until_choice(&mut engine);
    let valid = rv1::SubmitResolutionChoice {
        chosen_object_ids: vec![choice.candidate_object_ids[0]],
        ..Default::default()
    };
    let mut malformed = Vec::new();
    let mut answer = valid.clone();
    answer.decision = rv1::ResolutionChoiceDecision::Decline as i32;
    malformed.push(answer);
    let mut answer = valid.clone();
    answer.selected_branch_index = 1;
    malformed.push(answer);
    let mut answer = valid.clone();
    answer.cast_spell = Some(Default::default());
    malformed.push(answer);
    let mut answer = valid.clone();
    answer.chosen_combat_defender = Some(Default::default());
    malformed.push(answer);
    let mut answer = valid.clone();
    answer.payment = Some(Default::default());
    malformed.push(answer);
    let mut answer = valid.clone();
    answer.restricted_mana.push(Default::default());
    malformed.push(answer);
    let mut answer = valid;
    answer.spell_cast_announcement = Some(Default::default());
    malformed.push(answer);
    for answer in malformed {
        let before = engine.diagnostic_snapshot().unwrap();
        let command = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
        };
        assert!(engine.apply_command(4, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
}

#[test]
fn serialized_draw_order_and_brainstorm_completion_replay_partial_hands_identically() {
    fn fresh() -> GameEngine {
        let mut engine = setup(506_013);
        inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        inject_permanent_on_battlefield(&mut engine, 0, "teferis_ageless_insight");
        inject_card_into_hand(&mut engine, 0, "brainstorm");
        give_mana(
            &mut engine,
            4,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        engine
    }
    let mut engine = fresh();
    let mut commands = Vec::new();
    let slot = hand_index_for_card(&engine, 0, "brainstorm");
    let cast = cast_spell(slot, Vec::new());
    commands.push((4, cast.clone(), engine.apply_command(4, &cast).unwrap()));
    for _ in 0..30 {
        let (actor, command) = if let Some(pending) = engine.state.pending_resolution.as_ref() {
            let ids = if pending.presentation.choice_kind == rv1::ChoiceKind::ReplacementEffect {
                vec![pending.presentation.candidates[0]]
            } else {
                pending.presentation.candidates[..2].to_vec()
            };
            (pending.deciding_player, submit_resolution_choice(ids))
        } else if !engine.state.stack.is_empty() {
            (engine.state.priority_player_id(), pass())
        } else {
            break;
        };
        let batch = engine.apply_command(actor, &command).unwrap();
        commands.push((actor, command, batch));
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(commands
        .iter()
        .any(|(_, _, batch)| find_resolution_choice(batch)
            .is_some_and(|choice| choice.choice_kind() == rv1::ChoiceKind::ReplacementEffect)));
    let mut replay = fresh();
    for (actor, command, batch) in commands {
        let encoded = command.encode_to_vec();
        let decoded = rv1::RuledCommand::decode(encoded.as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), batch);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn departed_draw_step_decider_and_departed_spell_caster_never_strand_replacement_work() {
    for draw_step in [false, true] {
        let mut engine = setup(506_014);
        inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        let spell = if draw_step {
            engine.state.turn_step = TurnStep::Upkeep;
            engine.state.priority_idx = 0;
            None
        } else {
            Some(paid_spell(&mut engine, "divination"))
        };
        let choice = resolve_until_choice(&mut engine);
        assert_eq!(choice.deciding_player_id, 4);
        let before = engine
            .state
            .players
            .iter()
            .map(|player| player.hand.len())
            .collect::<Vec<_>>();
        engine.apply_command(4, &concede()).unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[1].hand.len(), before[1]);
        assert_eq!(engine.state.players[2].hand.len(), before[2]);
        if let Some(spell) = spell {
            assert!(!engine.state.stack.iter().any(|item| item.id == spell));
        }
        assert!(!engine.state.players[engine.state.priority_idx].has_lost);
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
}

#[test]
fn discard_then_draw_preserves_mandatory_empty_hand_and_optional_accept_decline_contracts() {
    for (mandatory, empty, accept) in [
        (true, false, true),
        (true, true, true),
        (false, false, true),
        (false, false, false),
        (false, true, false),
    ] {
        let mut engine = setup(506_015);
        inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        if empty {
            engine.state.players[0].hand.clear();
        }
        let card = if mandatory {
            "romantic_rendezvous"
        } else {
            "abandon_attachments"
        };
        let spell = inject_card_into_hand(&mut engine, 0, card);
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, card);
        engine
            .apply_command(4, &cast_spell(slot, Vec::new()))
            .unwrap();
        let before = engine.state.players[0].hand.len();
        let library = engine.state.players[0].library.len();
        if !mandatory && empty {
            resolve(&mut engine, spell);
        } else {
            let mut choice = resolve_until_choice(&mut engine);
            if !empty {
                assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
                assert_eq!(engine.state.players[0].library.len(), library);
                let ids = if accept {
                    vec![choice.candidate_object_ids[0]]
                } else {
                    Vec::new()
                };
                let batch = engine
                    .apply_command(4, &submit_resolution_choice(ids))
                    .unwrap();
                if accept {
                    choice = find_resolution_choice(&batch).unwrap().clone();
                } else {
                    assert!(find_resolution_choice(&batch).is_none());
                }
            }
            if mandatory || accept {
                assert!(answer_replacements(&mut engine, choice, "Thought Reflection").is_none());
            }
        }
        let drawn = if mandatory || accept { 8 } else { 0 };
        let discarded = usize::from(!empty && accept);
        assert_eq!(
            engine.state.players[0].hand.len(),
            before + drawn - discarded
        );
        assert_eq!(engine.state.players[0].library.len(), library - drawn);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn library_of_leng_destination_finishes_before_discard_tail_replacements_and_draws_returned_incarnation(
) {
    let mut engine = setup(506_016);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let returned = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let generation = engine
        .state
        .zone_change_generation
        .get(&returned)
        .copied()
        .unwrap_or(0);
    let spell = inject_card_into_hand(&mut engine, 0, "abandon_attachments");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "abandon_attachments");
    engine
        .apply_command(4, &cast_spell(slot, Vec::new()))
        .unwrap();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    let batch = engine
        .apply_command(4, &submit_resolution_choice(vec![returned]))
        .unwrap();
    let destination = find_resolution_choice(&batch).unwrap();
    assert_eq!(
        destination.choice_kind(),
        rv1::ChoiceKind::PrivateReplacement
    );
    assert_eq!(engine.state.objects[&returned].zone, Zone::Hand);
    let batch = engine
        .apply_command(4, &submit_resolution_choice(vec![1]))
        .unwrap();
    assert_eq!(engine.state.objects[&returned].zone, Zone::Library);
    assert_eq!(
        engine.state.zone_change_generation[&returned],
        generation + 1
    );
    assert_eq!(engine.state.players[0].library.front(), Some(&returned));
    let draw = find_resolution_choice(&batch).unwrap().clone();
    assert!(answer_replacements(&mut engine, draw, "Thought Reflection").is_none());
    assert!(engine.state.players[0].hand.contains(&returned));
    assert_eq!(
        engine.state.zone_change_generation[&returned],
        generation + 2
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn failed_replacement_leaves_count_only_success_and_finish_discard_and_authored_tail_before_loss() {
    let mut engine = setup(506_017);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    let observer = inject_permanent_on_battlefield(&mut engine, 0, "ominous_seas");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "island");
    engine.state.objects.get_mut(&land).unwrap().tapped = true;
    engine.state.players[0].library.truncate(1);
    let drawn_before = engine.state.turn_history.current.player(4).cards_drawn;
    paid_spell(&mut engine, "frantic_search");
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::ReplacementEffect);
    let discard = answer_replacements(&mut engine, choice, "Thought Reflection").unwrap();
    assert_eq!(discard.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!(
        engine.state.turn_history.current.player(4).cards_drawn,
        drawn_before + 1
    );
    assert!(engine.state.players[0].pending_library_loss);
    assert!(!engine.state.players[0].has_lost);
    assert_eq!(
        engine
            .state
            .staged_trigger_groups
            .iter()
            .flat_map(|group| &group.triggers)
            .filter(|trigger| trigger.source_permanent_id == observer)
            .count(),
        1,
        "failed replacement leaves cannot manufacture draw triggers"
    );
    let batch = engine
        .apply_command(
            4,
            &submit_resolution_choice(discard.candidate_object_ids[..2].to_vec()),
        )
        .unwrap();
    let lands = find_resolution_choice(&batch).unwrap();
    assert!(lands.candidate_object_ids.contains(&land));
    assert!(!engine.state.players[0].has_lost);
    engine
        .apply_command(4, &submit_resolution_choice(vec![land]))
        .unwrap();
    assert!(!engine.state.objects[&land].tapped);
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.players[engine.state.priority_idx].has_lost);
}

#[test]
fn caster_departure_cancels_foreign_drawer_fanout_without_replaying_prior_draws() {
    let mut engine = setup(506_018);
    let jace = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    inject_permanent_on_battlefield(&mut engine, 1, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 1, "thought_reflection");
    engine
        .apply_command(4, &activate_ability(jace, 0, Vec::new()))
        .unwrap();
    let before = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.len())
        .collect::<Vec<_>>();
    let choice = resolve_until_choice(&mut engine);
    assert_eq!(choice.deciding_player_id, 9);
    assert_eq!(engine.state.players[0].hand.len(), before[0] + 1);
    engine.apply_command(4, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[1].hand.len(), before[1]);
    assert_eq!(engine.state.players[2].hand.len(), before[2]);
    let snapshot = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(
            9,
            &submit_resolution_choice(vec![choice.candidate_object_ids[0]])
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), snapshot);
}

#[test]
fn stale_source_incarnation_rejects_then_new_incarnation_can_apply_to_child_event() {
    let mut engine = setup(506_019);
    let a = inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    for _ in 0..20 {
        inject_library_card(&mut engine, 0, "island");
    }
    paid_spell(&mut engine, "divination");
    let first = resolve_until_choice(&mut engine);
    let chosen = first
        .replacement_options
        .iter()
        .find(|option| option.source_object_id == a)
        .unwrap()
        .application_id;
    let batch = engine
        .apply_command(4, &submit_resolution_choice(vec![chosen]))
        .unwrap();
    let child = find_resolution_choice(&batch).unwrap().clone();
    let stale = child.replacement_options[0].clone();
    *engine
        .state
        .zone_change_generation
        .entry(stale.source_object_id)
        .or_default() += 2;
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(4, &submit_resolution_choice(vec![stale.application_id]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    *engine
        .state
        .zone_change_generation
        .get_mut(&stale.source_object_id)
        .unwrap() -= 2;
    *engine.state.zone_change_generation.entry(a).or_default() += 2;
    let batch = engine
        .apply_command(4, &submit_resolution_choice(vec![stale.application_id]))
        .unwrap();
    let next = find_resolution_choice(&batch).unwrap();
    assert!(next
        .replacement_options
        .iter()
        .any(|option| option.source_object_id == a));
}

#[test]
fn face_down_basic_land_suppression_and_current_control_scope_draw_replacements() {
    for mode in 0..3 {
        let mut engine = setup(506_020);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        if mode == 0 {
            engine.state.objects.get_mut(&source).unwrap().face_down = true;
        } else {
            let kind = if mode == 1 {
                ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                    card_types: vec![tricerules_cards::PermanentTypeFilter::Land],
                    creature_types: Vec::new(),
                    land_types: vec![BasicLandType::Forest],
                })
            } else {
                ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(9),
                }
            };
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: None,
                trigger_grant_origin: None,
                affected: AffectedScope::Single(source),
                kind,
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
        }
        let spell = paid_spell(&mut engine, "divination");
        let before = engine.state.players[0].hand.len();
        resolve(&mut engine, spell);
        assert_eq!(engine.state.players[0].hand.len(), before + 2);
        if mode == 0 {
            engine.state.objects.get_mut(&source).unwrap().face_down = false;
        } else {
            engine.state.continuous_effects.clear();
        }
        let spell = paid_spell(&mut engine, "divination");
        let before = engine.state.players[0].hand.len();
        resolve(&mut engine, spell);
        assert_eq!(engine.state.players[0].hand.len(), before + 4);
    }
}

#[test]
fn whole_hand_discard_library_top_order_completes_before_expanded_draw_and_adventure_exit() {
    let mut engine = setup(506_021);
    engine.state.players[0].hand.clear();
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let first = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let second = inject_card_into_hand(&mut engine, 0, "mountain");
    let discarded = inject_card_into_hand(&mut engine, 0, "opt");
    let prefix = engine.state.players[0]
        .library
        .iter()
        .take(6)
        .copied()
        .collect::<Vec<_>>();
    let adventure = inject_card_into_hand(&mut engine, 0, "hearth_elemental_stoke_genius");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "hearth_elemental_stoke_genius");
    engine
        .apply_command(4, &cast_spell_face(slot, Vec::new(), 1))
        .unwrap();
    let destination = resolve_until_choice(&mut engine);
    assert_eq!(
        destination.choice_kind(),
        rv1::ChoiceKind::PrivateReplacement
    );
    for selected in [1, 1, 0] {
        engine
            .apply_command(4, &submit_resolution_choice(vec![selected]))
            .unwrap();
    }
    assert_eq!(engine.state.objects[&adventure].zone, Zone::Stack);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(9, &submit_resolution_choice(vec![second, first]))
        .is_err());
    assert!(engine
        .apply_command(4, &submit_resolution_choice(vec![first, first]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let batch = engine
        .apply_command(4, &submit_resolution_choice(vec![second, first]))
        .unwrap();
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>(),
        vec![second, first]
    );
    let draw = find_resolution_choice(&batch).unwrap().clone();
    assert!(answer_replacements(&mut engine, draw, "Thought Reflection").is_none());
    let mut expected = vec![second, first];
    expected.extend(prefix);
    assert_eq!(engine.state.players[0].hand, expected);
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&adventure].zone, Zone::Exile);
}

#[test]
fn foreign_source_departure_finishes_scheduled_draw_without_repeating_step_action() {
    let mut engine = setup(506_022);
    inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
    let foreign = inject_permanent_on_battlefield(&mut engine, 1, "thought_reflection");
    engine
        .state
        .objects
        .get_mut(&foreign)
        .unwrap()
        .base_controller = 4;
    engine.state.objects.get_mut(&foreign).unwrap().controller = 4;
    engine.state.players[1]
        .battlefield
        .retain(|id| *id != foreign);
    engine.state.players[0].battlefield.push(foreign);
    engine.state.turn_step = TurnStep::Upkeep;
    engine.state.priority_idx = 0;
    let before = engine.state.players[0].hand.len();
    resolve_until_choice(&mut engine);
    engine.apply_command(9, &concede()).unwrap();
    assert_eq!(engine.state.turn_step, TurnStep::Draw);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), before + 2);
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.players[0].hand.len(), before + 2);
}
