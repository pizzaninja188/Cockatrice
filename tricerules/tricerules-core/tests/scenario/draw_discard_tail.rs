//! Focused actual-card regressions for the remaining authored Draw/Discard tail.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

fn blight_cost_selection(
    cost_index: u32,
    object_id: u32,
    zone_change_generation: u64,
) -> rv1::CostSelection {
    rv1::CostSelection {
        cost_index,
        selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
            rv1::CostObjectRefs {
                objects: vec![rv1::CostObjectRef {
                    object_id,
                    zone_change_generation,
                }],
            },
        )),
    }
}

fn put_library_cards_on_top(engine: &mut GameEngine, player: usize, cards: &[&str]) -> Vec<u32> {
    let objects = cards
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !objects.contains(object_id));
    for object_id in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    objects
}

#[test]
fn gristle_glutton_draws_only_after_an_actual_discard() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        614_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["gristle_glutton", "grizzly_bears"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("Gristle Glutton engine");
    advance_to_main1_from_game_start(&mut engine);
    let glutton = relocate_to_battlefield(&mut engine, 0, "gristle_glutton", false);
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    // Move the opening hand out through the fixture state before activation, leaving a live
    // library so an erroneous draw is observable and does not turn into a draw-loss case.
    let opening_hand = std::mem::take(&mut engine.state.players[0].hand);
    for object_id in opening_hand {
        engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("hand object")
            .zone = Zone::Graveyard;
        engine.state.players[0].graveyard.push(object_id);
    }
    assert!(engine.state.players[0].hand.is_empty());
    let library_before = engine.state.players[0].library.len();
    let generation = engine
        .state
        .zone_change_generation
        .get(&recipient)
        .copied()
        .unwrap_or(0);

    engine
        .apply_scenario_command(
            0,
            &activate_ability_with_costs(
                glutton,
                0,
                vec![],
                vec![blight_cost_selection(1, recipient, generation)],
            ),
        )
        .expect("pay Gristle Glutton's Tap and Blight costs");
    resolve_entire_stack_two_player(&mut engine);

    assert!(engine.state.players[0].hand.is_empty());
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before,
        "without an actual discard, the conditional draw does not happen"
    );
    assert!(engine.state.objects[&glutton].tapped);
    assert_eq!(
        engine.state.objects[&recipient]
            .counter_count(tricerules_cards::CounterKind::MinusOneMinusOne),
        1
    );
}

#[test]
fn gristle_glutton_counts_a_discard_replaced_to_a_hidden_zone() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        614_002,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "forest",
                &["gristle_glutton", "grizzly_bears", "island", "forest"],
            ),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("Gristle Glutton engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let glutton = relocate_to_battlefield(&mut engine, 0, "gristle_glutton", false);
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    let discarded = engine.state.players[0].hand[hand_index_for_card(&engine, 0, "grizzly_bears")];
    let generation = engine
        .state
        .zone_change_generation
        .get(&recipient)
        .copied()
        .unwrap_or(0);
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    engine
        .apply_scenario_command(
            0,
            &activate_ability_with_costs(
                glutton,
                0,
                vec![],
                vec![blight_cost_selection(1, recipient, generation)],
            ),
        )
        .expect("pay Gristle Glutton's Tap and Blight costs");
    pass_both_players(&mut engine);

    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Gristle Glutton's mandatory discard choice");
    assert_eq!((discard.presentation.min, discard.presentation.max), (1, 1));
    assert!(discard.presentation.candidates.contains(&discarded));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("choose the card to discard");

    let replacement = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Library of Leng replacement destination");
    assert_eq!(
        replacement.presentation.choice_kind,
        ChoiceKind::PrivateReplacement
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![1]))
        .expect("put the discarded card on top of the library");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&discarded));
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "a discard replaced to the library still satisfies Gristle Glutton's draw condition"
    );
    assert_eq!(
        engine.state.objects[&recipient]
            .counter_count(tricerules_cards::CounterKind::MinusOneMinusOne),
        1
    );
}

#[test]
fn stormshriek_feral_front_face_keeps_flying_haste_and_activates_its_self_pump() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        614_009,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("Stormshriek Feral engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "stormshriek_feral_flush_out");
    let feral = put_creature_on_battlefield(&mut engine, 0, "stormshriek_feral_flush_out");

    assert!(engine.effective_has_keyword(feral, Keyword::Flying));
    assert!(engine.effective_has_keyword(feral, Keyword::Haste));
    assert_eq!(engine.effective_power(feral), Some(3));
    assert_eq!(engine.effective_toughness(feral), Some(3));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &activate_ability(feral, 0, vec![]))
        .expect("pay {1}{R} for Stormshriek Feral's source-bound pump");
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    pass_both_players(&mut engine);

    assert_eq!(engine.effective_power(feral), Some(4));
    assert_eq!(engine.effective_toughness(feral), Some(3));
    assert!(engine.effective_has_keyword(feral, Keyword::Flying));
    assert!(engine.effective_has_keyword(feral, Keyword::Haste));
}

#[test]
fn flush_out_omen_discards_draws_then_shuffles_after_successful_resolution() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        614_003,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("Flush Out Omen engine");
    advance_to_main1_from_game_start(&mut engine);
    let omen = inject_card_into_hand(&mut engine, 0, "stormshriek_feral_flush_out");
    let discarded = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let drawn = put_library_cards_on_top(&mut engine, 0, &["island", "mountain"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );
    let hand_before_cast = engine.state.players[0].hand.len();
    let library_before_cast = engine.state.players[0].library.len();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    let slot = hand_index_for_card(&engine, 0, "stormshriek_feral_flush_out");

    engine
        .apply_scenario_command(0, &cast_spell_face(slot, vec![], 1))
        .expect("cast Flush Out as an Omen");
    let choice_batch = pass_both_players_with_batch(&mut engine);
    let choice = find_resolution_choice(&choice_batch).expect("mandatory discard");
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.candidate_object_ids.contains(&discarded));
    assert_eq!(engine.state.players[0].library.len(), library_before_cast);

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard before drawing two");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert!(drawn
        .iter()
        .all(|object_id| engine.state.players[0].hand.contains(object_id)));
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 2
    );
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before_cast - 1
    );
    assert_eq!(engine.state.objects[&omen].zone, Zone::Library);
    assert!(engine.state.players[0].library.contains(&omen));
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_cast,
        "one card left hand as the spell, one was discarded, and two were drawn"
    );
    let omen_resolved_index = completion
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(rv1::ruled_event::Ev::StackResolved(value))
                    if value.object_id == omen
                        && value.destination == rv1::StackResolveDestination::Library as i32
            )
        })
        .expect("successful Omen resolution event into its owner's library");
    let shuffle_log = "P0 shuffles the resolving Omen into P0's library.";
    let shuffle_indices = completion
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match &event.ev {
            Some(rv1::ruled_event::Ev::Log(log)) if log.text == shuffle_log => Some(index),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        shuffle_indices.len(),
        1,
        "one successful Omen shuffle is logged"
    );
    assert!(
        shuffle_indices[0] > omen_resolved_index,
        "the Omen is reported in its owner's library before the shuffle event"
    );
}

#[test]
fn countered_flush_out_omen_never_applies_its_discard_draw_or_shuffle_program() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        614_004,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("countered Flush Out Omen engine");
    advance_to_main1_from_game_start(&mut engine);
    let omen = inject_card_into_hand(&mut engine, 0, "stormshriek_feral_flush_out");
    inject_card_into_hand(&mut engine, 1, "counterspell");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.clone();
    let library_before = engine.state.players[0].library.len();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    let omen_slot = hand_index_for_card(&engine, 0, "stormshriek_feral_flush_out");

    engine
        .apply_scenario_command(0, &cast_spell_face(omen_slot, vec![], 1))
        .expect("cast Flush Out as an Omen");
    engine
        .apply_scenario_command(0, &pass())
        .expect("Omen caster passes");
    let counter_slot = hand_index_for_card(&engine, 1, "counterspell");
    engine
        .apply_scenario_command(1, &cast_spell(counter_slot, target_object(omen)))
        .expect("counter Flush Out");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&omen].zone, Zone::Graveyard);
    assert!(!engine.state.players[0].library.contains(&omen));
    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert_eq!(
        engine.state.players[0].hand,
        hand_before
            .into_iter()
            .filter(|id| *id != omen)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn focus_the_mind_draws_three_then_finishes_one_required_discard() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        614_005,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("Focus the Mind engine");
    advance_to_main1_from_game_start(&mut engine);
    let spell = inject_card_into_hand(&mut engine, 0, "focus_the_mind");
    let discarded = inject_card_into_hand(&mut engine, 0, "hill_giant");
    let drawn = put_library_cards_on_top(&mut engine, 0, &["island", "mountain", "forest"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 5,
            ..Default::default()
        },
    );
    let hand_before_cast = engine.state.players[0].hand.len();
    let library_before_cast = engine.state.players[0].library.len();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    let slot = hand_index_for_card(&engine, 0, "focus_the_mind");

    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Focus the Mind");
    let choice_batch = pass_both_players_with_batch(&mut engine);
    let choice = find_resolution_choice(&choice_batch).expect("required discard after three draws");
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.candidate_object_ids.contains(&discarded));
    assert!(drawn
        .iter()
        .all(|object_id| choice.candidate_object_ids.contains(object_id)));
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before_cast - 3
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast - 1 + 3);

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("finish the draw-then-discard spell");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert!(drawn
        .iter()
        .all(|object_id| engine.state.players[0].hand.contains(object_id)));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before_cast - 3
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast + 1);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 3
    );
}

#[test]
fn melded_moxite_optional_discard_accepts_and_declines_with_exact_counts() {
    for accept in [false, true] {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            614_006 + u64::from(accept),
            &[0, 1],
            20,
            None,
            true,
        )
        .expect("Melded Moxite engine");
        advance_to_main1_from_game_start(&mut engine);
        let moxite = inject_card_into_hand(&mut engine, 0, "melded_moxite");
        let discarded = inject_card_into_hand(&mut engine, 0, "hill_giant");
        let drawn = put_library_cards_on_top(&mut engine, 0, &["island", "mountain"]);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 2,
                ..Default::default()
            },
        );
        let moxite_slot = hand_index_for_card(&engine, 0, "melded_moxite");
        engine
            .apply_scenario_command(0, &cast_spell(moxite_slot, vec![]))
            .expect("cast Melded Moxite");
        pass_both_players(&mut engine);
        assert_eq!(engine.state.objects[&moxite].zone, Zone::Battlefield);

        let hand_before_trigger = engine.state.players[0].hand.len();
        let library_before_trigger = engine.state.players[0].library.len();
        let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

        let choice_batch = pass_both_players_with_batch(&mut engine);
        let choice = find_resolution_choice(&choice_batch).expect("optional discard trigger");
        assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
        assert_eq!((choice.min, choice.max), (0, 1));
        assert!(choice.candidate_object_ids.contains(&discarded));
        engine
            .apply_scenario_command(
                0,
                &submit_resolution_choice(if accept { vec![discarded] } else { vec![] }),
            )
            .expect("answer optional discard");

        let draw_count = if accept { 2 } else { 0 };
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.objects[&moxite].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.players[0].library.len(),
            library_before_trigger - draw_count
        );
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before + draw_count as u32
        );
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before_trigger + draw_count - usize::from(accept)
        );
        assert_eq!(
            engine.state.objects[&discarded].zone,
            if accept { Zone::Graveyard } else { Zone::Hand }
        );
        assert_eq!(
            drawn
                .iter()
                .filter(|object_id| engine.state.players[0].hand.contains(object_id))
                .count(),
            draw_count
        );
    }
}
