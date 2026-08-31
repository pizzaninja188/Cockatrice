use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_event::Ev, ResolutionChoiceDecision};

fn put_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let objects = card_ids
        .iter()
        .map(|card_id| take_oid_from_library_or_hand(engine, player, card_id))
        .collect::<Vec<_>>();
    for object_id in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
        engine.state.objects.get_mut(object_id).unwrap().zone = Zone::Library;
    }
    objects
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn setup_divert(seed: u64, payable: bool) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["lightning_bolt"]),
            deck_with("island", &["divert_disaster"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    ensure_in_hand(&mut engine, 1, "divert_disaster");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: if payable { 2 } else { 0 },
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_player(1)))
        .unwrap();
    let bolt_oid = engine.state.stack.last().unwrap().id;
    engine.apply_command(0, &pass()).unwrap();
    let divert = hand_index_for_card(&engine, 1, "divert_disaster");
    engine
        .apply_command(1, &cast_spell(divert, target_object(bolt_oid)))
        .unwrap();
    engine.apply_command(1, &pass()).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    (engine, bolt_oid)
}

#[test]
fn issue_159_divert_disaster_uses_the_committed_payment_receipt() {
    let (mut paid, bolt) = setup_divert(159_001, true);
    let paid_result = paid
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::PayMana),
        )
        .unwrap();
    let lander = token_created_events(&paid_result)
        .into_iter()
        .find(|created| created.card_id == "lander")
        .expect("paid Divert Disaster creates a Lander token");
    assert_eq!(
        lander.identity.as_ref().unwrap().ability_texts,
        vec!["Lander — activated ability (activated_01)"]
    );
    assert!(paid.state.stack.iter().any(|item| item.id == bolt));
    assert_eq!(
        paid.state.players[1]
            .battlefield
            .iter()
            .filter(|oid| paid.state.objects[oid].card_id == "lander")
            .count(),
        1
    );

    let (mut declined, bolt) = setup_divert(159_002, false);
    declined
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .unwrap();
    assert_eq!(declined.state.objects[&bolt].zone, Zone::Graveyard);
    assert!(!declined.state.players[1]
        .battlefield
        .iter()
        .any(|oid| declined.state.objects[oid].card_id == "lander"));
}

#[test]
fn issue_159_lander_ability_stack_preserves_its_token_display_identity() {
    let (mut engine, _) = setup_divert(159_016, true);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::PayMana),
        )
        .unwrap();
    let lander = battlefield_token_oids(&engine, 1, "lander")
        .into_iter()
        .next()
        .expect("paid Divert Disaster creates a Lander token");

    engine.apply_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let activated = apply_ability(&mut engine, 1, lander, 0, vec![]).unwrap();
    let pushed = activated
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(pushed)) if pushed.object_id != 0 => Some(pushed),
            _ => None,
        })
        .expect("Lander activation reaches the stack");
    let identity = pushed
        .source_token_identity
        .as_ref()
        .expect("token ability stack item keeps the source display identity");

    assert_eq!(pushed.description, "Lander");
    assert_eq!(identity.name, "Lander");
    assert_eq!(identity.types, vec!["Artifact", "Lander"]);
    assert_eq!(
        identity.ability_texts,
        vec!["Lander — activated ability (activated_01)"]
    );
    assert!(!engine.state.players[1].battlefield.contains(&lander));
}

#[test]
fn issue_159_divert_rejects_unpayable_and_stale_payment_submissions_without_double_resume() {
    let (mut engine, bolt) = setup_divert(159_013, false);
    assert!(engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::PayMana),
        )
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.stack.iter().any(|item| item.id == bolt));

    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .unwrap();
    assert_eq!(engine.state.objects[&bolt].zone, Zone::Graveyard);
    let stack_len = engine.state.stack.len();
    assert!(engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .is_err());
    assert_eq!(engine.state.stack.len(), stack_len);
    assert!(!engine.state.players[1]
        .battlefield
        .iter()
        .any(|oid| engine.state.objects[oid].card_id == "lander"));
}

fn resolve_targeted_spell(engine: &mut GameEngine, caster: i32, card_id: &str, target: u32) {
    ensure_in_hand(engine, caster as usize, card_id);
    let slot = hand_index_for_card(engine, caster as usize, card_id);
    engine
        .apply_command(caster, &cast_spell(slot, target_object(target)))
        .unwrap();
    pass_both_players(engine);
}

#[test]
fn issue_159_depressurize_rechecks_the_same_targets_current_power() {
    let mut engine = GameEngine::new(
        159_003,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["depressurize", "depressurize"]),
            deck_with("forest", &["grizzly_bears", "colossal_dreadmaw"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    resolve_targeted_spell(&mut engine, 0, "depressurize", bear);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);

    ensure_in_hand(&mut engine, 0, "depressurize");
    let dreadmaw = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    resolve_targeted_spell(&mut engine, 0, "depressurize", dreadmaw);
    assert_eq!(engine.state.objects[&dreadmaw].zone, Zone::Battlefield);
}

#[test]
fn issue_159_yip_yip_does_not_narrow_initial_target_legality() {
    let mut engine = GameEngine::new(
        159_004,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "plains",
                &[
                    "yip_yip!",
                    "yip_yip!",
                    "avatar_enthusiasts",
                    "grizzly_bears",
                ],
            ),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let ally = relocate_to_battlefield(&mut engine, 0, "avatar_enthusiasts", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    resolve_targeted_spell(&mut engine, 0, "yip_yip!", bear);
    assert!(!engine.effective_has_keyword(bear, Keyword::Flying));
    resolve_targeted_spell(&mut engine, 0, "yip_yip!", ally);
    assert!(engine.effective_has_keyword(ally, Keyword::Flying));
}

#[test]
fn issue_159_midnight_tilling_offers_only_surviving_cards_milled_this_way() {
    let mut engine = GameEngine::new(
        159_005,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "forest",
                &[
                    "midnight_tilling",
                    "grizzly_bears",
                    "lightning_bolt",
                    "island",
                    "colossal_dreadmaw",
                ],
            ),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "midnight_tilling");
    let older = take_oid_from_library_or_hand(&mut engine, 0, "colossal_dreadmaw");
    engine.state.players[0].graveyard.push(older);
    engine.state.objects.get_mut(&older).unwrap().zone = Zone::Graveyard;
    let milled = put_on_top(
        &mut engine,
        0,
        &["grizzly_bears", "lightning_bolt", "island", "forest"],
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "midnight_tilling");
    engine.apply_command(0, &cast_spell(spell, vec![])).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::GraveyardCards);
    assert_eq!(
        pending.presentation.candidates,
        vec![milled[0], milled[2], milled[3]]
    );
    assert!(!pending.presentation.candidates.contains(&older));
    assert!(!pending.presentation.candidates.contains(&milled[1]));

    let chosen = milled[2];
    engine
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
        .unwrap();
    assert_eq!(engine.state.objects[&chosen].zone, Zone::Hand);
}

fn cast_blight(object_id: u32, generation: u64) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        selected_object: Some(
            tricerules_proto::ruled::v1::cast_cost_group_selection::SelectedObject::PermanentId(
                object_id,
            ),
        ),
        expected_zone_change_generation: generation,
    }
}

#[test]
fn issue_159_burning_curiosity_uses_one_group_for_the_exact_two_or_three_cards() {
    for paid in [false, true] {
        let mut engine = GameEngine::new(
            159_006 + u64::from(paid),
            &[0, 1],
            20,
            Some(vec![
                deck_with(
                    "mountain",
                    &["burning_curiosity", "grizzly_bears", "forest", "island"],
                ),
                deck_with("island", &[]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        ensure_in_hand(&mut engine, 0, "burning_curiosity");
        let blight_creature = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        let top = put_on_top(&mut engine, 0, &["forest", "island", "mountain"]);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        );
        let spell = hand_index_for_card(&engine, 0, "burning_curiosity");
        let groups = if paid {
            let generation = engine
                .state
                .zone_change_generation
                .get(&blight_creature)
                .copied()
                .unwrap_or(0);
            vec![cast_blight(blight_creature, generation)]
        } else {
            Vec::new()
        };
        engine
            .apply_command(0, &cast_spell_with_cast_cost_groups(spell, vec![], groups))
            .unwrap();
        resolve_entire_stack_two_player(&mut engine);

        let expected = if paid { 3 } else { 2 };
        assert!(top[..expected]
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Exile));
        if !paid {
            assert_eq!(engine.state.objects[&top[2]].zone, Zone::Library);
        }
        let permissions = &engine.state.active_exile_play_permissions;
        assert_eq!(permissions.len(), expected);
        assert!(permissions
            .iter()
            .all(|permission| permission.group_id == permissions[0].group_id));
        assert!(permissions.iter().all(|permission| {
            permission.expires_at_cleanup_turn_instance
                == permissions[0].expires_at_cleanup_turn_instance
        }));
    }
}

#[test]
fn issue_159_lost_days_owner_places_the_exact_target_second_from_top() {
    let mut engine = GameEngine::new(
        159_008,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["lost_days"]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "lost_days");
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let previous_top = engine.state.players[1].library.front().copied().unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 4,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "lost_days");
    engine
        .apply_command(0, &cast_spell(spell, target_object(target)))
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    let resolved = engine.apply_command(1, &select_branch(0)).unwrap();

    assert_eq!(engine.state.players[1].library[0], previous_top);
    assert_eq!(engine.state.players[1].library[1], target);
    assert_eq!(engine.state.objects[&target].zone, Zone::Library);
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .any(|oid| engine.state.objects[oid].card_id == "clue"));
    let clue = token_created_events(&resolved)
        .into_iter()
        .find(|created| created.card_id == "clue")
        .expect("Lost Days creates a Clue token");
    assert_eq!(
        clue.identity.as_ref().unwrap().ability_texts,
        vec!["Clue — activated ability (activated_01)"]
    );
}

fn kicker_option() -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        selected_object: None,
        expected_zone_change_generation: 0,
    }
}

#[test]
fn issue_159_aangs_journey_enables_only_the_receipt_backed_search_slots() {
    for kicked in [false, true] {
        let mut engine = GameEngine::new(
            159_009 + u64::from(kicked),
            &[0, 1],
            20,
            Some(vec![
                deck_with("forest", &["aangs_journey"]),
                deck_with("island", &[]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        ensure_in_hand(&mut engine, 0, "aangs_journey");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: if kicked { 4 } else { 2 },
                ..Default::default()
            },
        );
        let basic = engine.state.players[0].library.front().copied().unwrap();
        let spell = hand_index_for_card(&engine, 0, "aangs_journey");
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    spell,
                    vec![],
                    kicked.then(kicker_option).into_iter().collect(),
                ),
            )
            .unwrap();
        engine.apply_command(0, &pass()).unwrap();
        engine.apply_command(1, &pass()).unwrap();

        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.presentation.min, 0);
        assert_eq!(pending.presentation.max, if kicked { 2 } else { 1 });
        let ResolutionContinuation::SearchLibrary {
            selection_slot_candidates,
            ..
        } = &pending.continuation
        else {
            panic!("Aang's Journey must park a library search");
        };
        assert_eq!(selection_slot_candidates.len(), if kicked { 2 } else { 1 });
        assert!(selection_slot_candidates[0].contains(&basic));
        if kicked {
            assert!(selection_slot_candidates[1].is_empty());
        }

        engine
            .apply_command(0, &submit_resolution_choice(vec![basic]))
            .unwrap();
        assert_eq!(engine.state.objects[&basic].zone, Zone::Hand);
        assert_eq!(engine.state.players[0].life, 22);
    }
}

#[test]
fn issue_159_library_search_rejects_a_stale_slot_candidate_atomically() {
    let mut engine = GameEngine::new(
        159_011,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["aangs_journey"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "aangs_journey");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let basic = engine.state.players[0].library.front().copied().unwrap();
    let spell = hand_index_for_card(&engine, 0, "aangs_journey");
    engine.apply_command(0, &cast_spell(spell, vec![])).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();

    *engine
        .state
        .zone_change_generation
        .entry(basic)
        .or_default() += 1;
    let library_before = engine.state.players[0].library.clone();
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![basic]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.players[0].library, library_before);
    assert_eq!(engine.state.players[0].life, 20);
}
