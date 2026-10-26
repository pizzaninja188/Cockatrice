use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use prost::Message;
use std::collections::BTreeSet;

/// Rejected typed-Gifts answers must be pure: the parked program frame/slots, exact game state,
/// command index, deterministic random inputs, and library ordering all remain unchanged.
fn assert_rejected_gifts_answer_is_immutable(
    engine: &mut GameEngine,
    actor: i32,
    command: &RuledCommand,
    label: &str,
) {
    let pending_before = format!("{:?}", engine.state.pending_resolution);
    let state_before = format!("{:?}", engine.state);
    let seed_before = engine.state.seed;
    let command_index_before = engine.state.command_index;
    let libraries_before = engine
        .state
        .players
        .iter()
        .map(|player| player.library.iter().copied().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert!(
        engine.apply_command(actor, command).is_err(),
        "{label} must reject"
    );
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_before
    );
    assert_eq!(format!("{:?}", engine.state), state_before);
    assert_eq!(engine.state.seed, seed_before);
    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.library.iter().copied().collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        libraries_before,
        "a rejected command must not consume or reorder library state"
    );
}

fn make_gifts_partition_prompt(
    seed: u64,
) -> (
    GameEngine,
    Vec<u32>,
    tricerules_proto::ruled::v1::ResolutionChoiceRequired,
) {
    let decks = Some(vec![
        {
            let mut deck = vec!["gifts_ungiven".to_string()];
            deck.extend(std::iter::repeat_n("island".to_string(), 29));
            deck
        },
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new Gifts stale-reference scenario");
    advance_to_main1_from_game_start(&mut engine);
    let found = ["lightning_bolt", "divination", "counterspell", "murder"]
        .iter()
        .map(|card_id| inject_library_card(&mut engine, 0, card_id))
        .collect::<Vec<_>>();
    let search_batch = cast_targeted_instant_and_resolve(
        &mut engine,
        0,
        "gifts_ungiven",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_player(1),
    );
    let search = find_resolution_choice(&search_batch).expect("Gifts search offer");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    let split_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(found.clone()))
        .expect("submit retained library cohort");
    let split = find_resolution_choice(&split_batch).expect("Gifts target split offer");
    (engine, found, split)
}

fn insert_authored_activated_owner(
    owners: &mut BTreeSet<tricerules_card_model::ProgramOwnerId>,
    card_id: &str,
    face_id: &tricerules_card_model::CardFaceId,
    ability_path: Vec<tricerules_card_model::AbilityId>,
    ability: &tricerules_card_model::ActivatedAbilityDef,
) {
    if ability.program.is_some() {
        owners.insert(tricerules_card_model::ProgramOwnerId::ability(
            tricerules_card_model::AbilityDefinitionId {
                card_id: card_id.to_owned(),
                face_id: face_id.clone(),
                ability_path,
            },
        ));
    }
}

fn insert_authored_triggered_owner(
    owners: &mut BTreeSet<tricerules_card_model::ProgramOwnerId>,
    card_id: &str,
    face_id: &tricerules_card_model::CardFaceId,
    ability_path: Vec<tricerules_card_model::AbilityId>,
    ability: &tricerules_card_model::TriggeredAbilityDef,
) {
    let definition = tricerules_card_model::AbilityDefinitionId {
        card_id: card_id.to_owned(),
        face_id: face_id.clone(),
        ability_path,
    };
    if let Some(modal) = &ability.modal {
        for mode in modal.modes.iter().filter(|mode| mode.program.is_some()) {
            owners.insert(tricerules_card_model::ProgramOwnerId::ability_mode(
                definition.clone(),
                mode.mode_id.clone(),
            ));
        }
    } else if ability.program.is_some() {
        owners.insert(tricerules_card_model::ProgramOwnerId::ability(definition));
    }
}

fn source_authored_program_owners() -> BTreeSet<tricerules_card_model::ProgramOwnerId> {
    use tricerules_card_model::ProgramOwnerId;

    let registry = tricerules_cards::registry::global();
    let mut owners = BTreeSet::new();
    for card in registry.definitions() {
        for face in &card.faces {
            if face.spell_program.is_some() {
                owners.insert(ProgramOwnerId::face(card.id.clone(), face.face_id.clone()));
            }
            if let Some(modal) = &face.modal_spell {
                for mode in modal.modes.iter().filter(|mode| mode.program.is_some()) {
                    owners.insert(ProgramOwnerId::face_mode(
                        card.id.clone(),
                        face.face_id.clone(),
                        mode.mode_id.clone(),
                    ));
                }
            }

            for ability in &face.activated_abilities {
                insert_authored_activated_owner(
                    &mut owners,
                    &card.id,
                    &face.face_id,
                    vec![ability.ability_id.clone()],
                    ability,
                );
            }
            for ability in &face.triggered_abilities {
                insert_authored_triggered_owner(
                    &mut owners,
                    &card.id,
                    &face.face_id,
                    vec![ability.ability_id.clone()],
                    ability,
                );
            }
            for bar in &face.class_level_bars {
                insert_authored_activated_owner(
                    &mut owners,
                    &card.id,
                    &face.face_id,
                    vec![bar.level_ability.ability_id.clone()],
                    &bar.level_ability,
                );
                for ability in &bar.activated_abilities {
                    insert_authored_activated_owner(
                        &mut owners,
                        &card.id,
                        &face.face_id,
                        vec![ability.ability_id.clone()],
                        ability,
                    );
                }
                for ability in &bar.triggered_abilities {
                    insert_authored_triggered_owner(
                        &mut owners,
                        &card.id,
                        &face.face_id,
                        vec![ability.ability_id.clone()],
                        ability,
                    );
                }
                for parent in &bar.static_abilities {
                    insert_static_authored_program_owners(
                        &mut owners,
                        &card.id,
                        &face.face_id,
                        parent,
                    );
                }
            }

            for parent in &face.static_abilities {
                insert_static_authored_program_owners(&mut owners, &card.id, &face.face_id, parent);
            }
        }
    }
    owners
}

fn insert_static_authored_program_owners(
    owners: &mut BTreeSet<tricerules_card_model::ProgramOwnerId>,
    card_id: &str,
    face_id: &tricerules_card_model::CardFaceId,
    parent: &tricerules_card_model::IdentifiedStaticAbility,
) {
    use tricerules_card_model::primitives::StaticAbilityDef;

    let parent_id = parent.ability_id.clone();
    let (activated, triggered) = match &parent.definition {
        StaticAbilityDef::AttachedModifier {
            activated_abilities,
            triggered_abilities,
            ..
        }
        | StaticAbilityDef::ConditionalSelfModifier {
            activated_abilities,
            triggered_abilities,
            ..
        } => (
            activated_abilities.as_slice(),
            triggered_abilities.as_slice(),
        ),
        StaticAbilityDef::GrantActivatedAbilityToPermanents {
            activated_abilities,
            ..
        } => (activated_abilities.as_slice(), &[][..]),
        StaticAbilityDef::GrantTriggeredAbilityToPermanents {
            triggered_abilities,
            ..
        }
        | StaticAbilityDef::ConditionalPermanentTransformation {
            triggered_abilities,
            ..
        } => (&[][..], triggered_abilities.as_slice()),
        _ => (&[][..], &[][..]),
    };
    for ability in activated {
        insert_authored_activated_owner(
            owners,
            card_id,
            face_id,
            vec![parent_id.clone(), ability.ability_id.clone()],
            ability,
        );
    }
    for ability in triggered {
        insert_authored_triggered_owner(
            owners,
            card_id,
            face_id,
            vec![parent_id.clone(), ability.ability_id.clone()],
            ability,
        );
    }
}

fn frozen_draw_discard_source_owners() -> BTreeSet<tricerules_card_model::ProgramOwnerId> {
    use tricerules_card_model::{
        AbilityDefinitionId, AbilityId, CardFaceId, ModeId, ProgramOwnerId,
    };

    let face_id = |value: &str| CardFaceId::new(value).expect("frozen face id is canonical");
    let face = |card_id: &str, face: &str| ProgramOwnerId::face(card_id, face_id(face));
    let face_mode = |card_id: &str, face: &str, mode: &str| {
        ProgramOwnerId::face_mode(
            card_id,
            face_id(face),
            ModeId::new(mode).expect("frozen mode id is canonical"),
        )
    };
    let ability = |card_id: &str, face: &str, ability: &str| {
        ProgramOwnerId::ability(AbilityDefinitionId {
            card_id: card_id.to_owned(),
            face_id: face_id(face),
            ability_path: vec![AbilityId::new(ability).expect("frozen ability id is canonical")],
        })
    };

    // Frozen from baseline 05d3fb97 source data plus the six exact owners migrated in this change.
    BTreeSet::from([
        face("abandon_attachments", "abandon_attachments"),
        face("ancestral_reminiscence", "ancestral_reminiscence"),
        face("broadside_barrage", "broadside_barrage"),
        face("calamitous_tide", "calamitous_tide"),
        face("careful_study", "careful_study"),
        face("focus_the_mind", "focus_the_mind"),
        face("incinerating_blast", "incinerating_blast"),
        face("refute", "refute"),
        face("romantic_rendezvous", "romantic_rendezvous"),
        face("stormshriek_feral_flush_out", "flush_out"),
        face("traumatic_critique", "traumatic_critique"),
        face_mode(
            "confusticate_and_bebother",
            "confusticate_and_bebother",
            "mode_01",
        ),
        face_mode(
            "confusticate_and_bebother",
            "confusticate_and_bebother",
            "mode_02",
        ),
        face_mode("three_steps_ahead", "three_steps_ahead", "copy"),
        face_mode("three_steps_ahead", "three_steps_ahead", "counter"),
        face_mode("three_steps_ahead", "three_steps_ahead", "draw_discard"),
        ability("agna_qela", "agna_qela", "activated_02"),
        ability("anvil_of_bogardan", "anvil_of_bogardan", "triggered_01"),
        ability("bellowing_crier", "bellowing_crier", "triggered_01"),
        ability(
            "currency_converter",
            "currency_converter",
            "draw_then_discard",
        ),
        ability("discerning_peddler", "discerning_peddler", "triggered_01"),
        ability("dreadwing_scavenger", "dreadwing_scavenger", "triggered_01"),
        ability("dreadwing_scavenger", "dreadwing_scavenger", "triggered_02"),
        ability("gristle_glutton", "gristle_glutton", "activated_01"),
        ability("icewind_elemental", "icewind_elemental", "triggered_01"),
        ability("keldon_raider", "keldon_raider", "triggered_01"),
        ability(
            "kiora,_the_rising_tide",
            "kiora_the_rising_tide",
            "triggered_01",
        ),
        ability("long_lake_nuisance", "long_lake_nuisance", "triggered_01"),
        ability("mechan_navigator", "mechan_navigator", "triggered_01"),
        ability("melded_moxite", "melded_moxite", "triggered_01"),
        ability(
            "null_group_biological_assets",
            "null_group_biological_assets",
            "triggered_01",
        ),
        ability("patient_instructor", "patient_instructor", "triggered_01"),
        ability("quicksmith_genius", "quicksmith_genius", "triggered_01"),
        ability("ragged_short_spear", "ragged_short_spear", "triggered_01"),
        ability("rescue_leopard", "rescue_leopard", "triggered_01"),
        ability("roiling_dragonstorm", "roiling_dragonstorm", "triggered_01"),
        ability("rousing_read", "rousing_read", "triggered_01"),
        ability("silvergill_peddler", "silvergill_peddler", "triggered_01"),
        ability("strix_lookout", "strix_lookout", "activated_01"),
        ability("teferis_protege", "teferi_s_protege", "activated_01"),
        ability("temur_tawnyback", "temur_tawnyback", "triggered_01"),
        ability(
            "the_mountain-kings_return",
            "the_mountain_kings_return",
            "chapter_i",
        ),
        ability("uthros_scanship", "uthros_scanship", "triggered_01"),
        ability(
            "volatile_wanderglyph",
            "volatile_wanderglyph",
            "triggered_01",
        ),
        ability("yuyan_archers", "yuyan_archers", "triggered_01"),
    ])
}

#[test]
fn frozen_draw_discard_owners_are_authored_and_compile_for_their_exact_identity() {
    let registry = tricerules_cards::registry::global();
    let frozen_owners = frozen_draw_discard_source_owners();
    let source_authored_owners = source_authored_program_owners();
    let authored_frozen_owners = frozen_owners
        .intersection(&source_authored_owners)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        authored_frozen_owners, frozen_owners,
        "every frozen source owner must now have an authored program"
    );
    for owner in &frozen_owners {
        let program = registry
            .compiled_program_for_owner(owner)
            .unwrap_or_else(|| panic!("missing compiled program for exact frozen owner {owner:?}"));
        assert_eq!(&program.owner, owner);
    }
}

#[test]
fn brainstorm_draws_three_then_returns_two_in_chosen_order() {
    let decks = Some(vec![
        {
            let mut d = vec!["brainstorm".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        42,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "brainstorm");

    let hand_before = e.state.players[0].hand.len();
    let lib_before = e.state.players[0].library.len();
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    // Parked on a choice; the spell drew three and asks for two back, in order.
    let req = find_resolution_choice(&batch).expect("resolution choice required");
    assert_eq!(req.deciding_player_id, 0);
    assert_eq!((req.min, req.max), (2, 2));
    assert!(req.ordered);
    assert_eq!(req.choice_kind(), ChoiceKind::HandCards);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("parked Brainstorm");
    let internal_prompt_id = pending.resolution_prompt_id.get();
    assert_ne!(
        internal_prompt_id, 0,
        "pending offer has a nonzero prompt ID"
    );
    assert_eq!(
        req.resolution_prompt_id, internal_prompt_id,
        "the published choice and installed pending offer share one ID"
    );
    let prompt_state = batch
        .resolution_prompt_state
        .as_ref()
        .expect("gameplay batch publishes its final prompt state");
    assert!(prompt_state.active);
    assert_eq!(prompt_state.deciding_player_id, 0);
    assert_eq!(prompt_state.resolution_prompt_id, Some(internal_prompt_id));
    assert!(
        matches!(
            &pending.continuation,
            ResolutionContinuation::HandChoice { hand_choice, .. }
                if hand_choice.program_selection
        ),
        "Brainstorm suspends through the typed program runner"
    );
    // Cast removed brainstorm from hand; begin drew three.
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1 + 3);
    assert_eq!(e.state.players[0].library.len(), lib_before - 3);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        3,
        "program draws report the same committed history as primitive draws"
    );

    // Put two specific hand cards on top: last chosen = top (intuitive "place A, then B on top").
    let chosen: Vec<u32> = e.state.players[0].hand.iter().take(2).copied().collect();
    let (first, second) = (chosen[0], chosen[1]);
    let completed = e
        .apply_scenario_command(0, &submit_resolution_choice(chosen))
        .expect("submit brainstorm choice");

    let prompt_state = completed
        .resolution_prompt_state
        .as_ref()
        .expect("completed gameplay batch publishes prompt retirement");
    assert!(!prompt_state.active);
    assert_eq!(prompt_state.resolution_prompt_id, None);
    assert!(e.state.pending_resolution.is_none(), "resolution completed");
    assert_eq!(e.state.players[0].library[0], second, "last chosen on top");
    assert_eq!(e.state.players[0].library[1], first, "first chosen below");
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1 + 3 - 2);
    assert_eq!(count_card_id_in_graveyard(&e, 0, "brainstorm"), 1);
}

#[test]
fn optional_exact_discard_requires_zero_or_the_full_payment_before_branching() {
    let program = r#"(
        id: "optional_exact_discard_program",
        name: "Optional Exact Discard Program",
        face_id: "optional_exact_discard_program",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "discard_two", bind: ["discard_result"], kind: Perform(
                    Discard(who: Controller, quantity: OptionalExact(2)))),
                (id: "draw_if_paid", kind: IfResult(
                    condition: DiscardOutcome(
                        result: "discard_result", outcome: ChosenToPay),
                    yes: (steps: [
                        (id: "paid_draw", kind: Perform(Draw(count: 1)))
                    ], yields: []),
                    no: (steps: [], yields: [])
                ))
            ],
            yields: []
        )))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("optional exact program fixture registry validates"),
    ));
    let decks = Some(vec![vec!["island".into(); 30], vec!["forest".into(); 30]]);
    let mut engine = GameEngine::new(registry, 9_427, &[0, 1], 20, decks, true)
        .expect("optional exact fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "optional_exact_discard_program");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "optional_exact_discard_program"),
                Vec::new(),
            ),
        )
        .expect("cast the zero-cost optional exact program");
    let first_passer = engine.state.priority_player_id();
    engine
        .apply_scenario_command(first_passer, &pass())
        .expect("first player passes priority");
    let second_passer = engine.state.priority_player_id();
    let prompt_batch = engine
        .apply_scenario_command(second_passer, &pass())
        .expect("second player passes and resolves the spell to its discard choice");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("optional exact discard prompts when two cards are available");
    let prompt = prompt_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .expect("discard choice publication");
    assert_eq!(prompt.min, 0);
    assert_eq!(prompt.max, 2);
    assert_eq!(
        prompt.selection_alternatives.len(),
        1,
        "OptionalExact has one nonzero exact-payment alternative; zero is represented by min = 0"
    );
    assert_eq!(prompt.selection_alternatives[0].count, 2);
    assert_eq!(
        prompt.selection_alternatives[0].candidate_indices,
        (0..prompt.candidate_object_ids.len() as u32).collect::<Vec<_>>(),
        "every eligible card is available for the exact payment"
    );
    assert_eq!(
        prompt.prompt_text,
        "P0: choose zero or exactly 2 cards to discard."
    );
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!((pending.presentation.min, pending.presentation.max), (0, 2));
    let candidates = pending.presentation.candidates.clone();
    assert!(candidates.len() >= 2);
    let draws_before_choice = engine.state.turn_history.current.player(0).cards_drawn;

    let before_partial = engine
        .diagnostic_snapshot()
        .expect("snapshot before partial answer");
    assert!(
        engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![candidates[0]]))
            .is_err(),
        "one card is neither a decline nor an exact-two payment"
    );
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after rejected partial answer"),
        before_partial,
        "partial optional payment rejects before mutating the prompt, hand, RNG, or command index"
    );

    engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice(candidates.iter().take(2).copied().collect()),
        )
        .expect("submit the exact-two payment");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before_choice + 1,
        "the typed ChosenToPay outcome draws once after the discard transaction commits"
    );
    assert!(candidates
        .iter()
        .take(2)
        .all(|object| engine.state.objects[object].zone == tricerules_core::Zone::Graveyard));

    inject_card_into_hand(&mut engine, 0, "optional_exact_discard_program");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "optional_exact_discard_program"),
                Vec::new(),
            ),
        )
        .expect("cast a second optional exact program to exercise decline");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the second optional exact program also offers a choice");
    assert_eq!((pending.presentation.min, pending.presentation.max), (0, 2));
    let hand_before_decline = engine.state.players[0].hand.clone();
    let draws_before_decline = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline by submitting no cards");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand, hand_before_decline);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before_decline,
        "the Declined outcome skips the paid-only branch"
    );
}

#[test]
fn authored_soft_counter_resumes_at_its_following_discard_after_payment() {
    let counter = r#"(
        id: "program_counter_then_loot",
        name: "Program Counter Then Loot",
        face_id: "program_counter_then_loot",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "counter", kind: Perform(CounterTargetSpell(
                spell_filter: (), unless_controller_pays: Some(1)
            ))),
            (id: "draw", kind: Perform(Draw(who: Controller, count: 1))),
            (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                who: Controller, quantity: Exact(1)
            )))
        ], yields: []), target_bindings: [
            (instruction_id: "counter", role: Spell, target_group_index: 0)
        ])),
        targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target spell",
            effect_indices: [])]))
    )"#;
    let target = r#"(
        id: "program_counter_target",
        name: "Program Counter Target",
        face_id: "program_counter_target",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [Draw(count: 1)]
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                counter,
                target,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("soft-counter program fixture registry validates"),
    ));
    let decks = Some(vec![
        deck_with("island", &["program_counter_target"]),
        deck_with("forest", &["program_counter_then_loot"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_431, &[0, 1], 20, decks, true)
        .expect("soft-counter program fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "program_counter_target");
    inject_card_into_hand(&mut engine, 1, "program_counter_then_loot");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "program_counter_target"),
                Vec::new(),
            ),
        )
        .expect("cast the spell that will be targeted");
    let target_spell = engine.state.stack.last().expect("target spell on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority to respond");
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "program_counter_then_loot"),
                vec![TargetRef {
                    object_id: target_spell,
                    kind: TargetRefKind::Stack as i32,
                    group_index: 0,
                    ..Default::default()
                }],
            ),
        )
        .expect("cast the source-lowered soft counter in response");
    let draws_before = engine.state.turn_history.current.player(1).cards_drawn;
    pass_priority_round(&mut engine);

    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the target controller may pay the soft-counter cost");
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::ManaPayment);
    assert_eq!(payment.deciding_player, 0);
    submit_mana_resolution_decision(
        &mut engine,
        0,
        tricerules_proto::ruled::v1::ResolutionChoiceDecision::PayMana,
    )
    .expect("pay to preserve the target spell");

    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the authored discard after payment must resume");
    assert_eq!(discard.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!(discard.deciding_player, 1);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == target_spell));
    let discarded = discard.presentation.candidates[0];
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("finish the resumed authored discard step");
    assert_eq!(
        engine.state.objects[&discarded].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == target_spell));
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        draws_before + 1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(
        engine.state.stack.len() == 1,
        "the target spell remains on stack"
    );
}

#[test]
fn authored_target_binding_publishes_casts_and_resumes_after_a_soft_counter_choice() {
    let counter = r#"(
        id: "authored_program_counter_then_draw",
        name: "Authored Program Counter Then Draw",
        face_id: "authored_program_counter_then_draw",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "counter", kind: Perform(CounterTargetSpell(
                spell_filter: (), unless_controller_pays: Some(1)))),
            (id: "draw", bind: ["drawn"], kind: Perform(Draw(count: 1))),
        ], yields: []), target_bindings: [
            (instruction_id: "counter", role: Spell, target_group_index: 0),
        ])),
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose target spell", effect_indices: [])]))
    )"#;
    let target = r#"(
        id: "authored_program_counter_target",
        name: "Authored Program Counter Target",
        face_id: "authored_program_counter_target",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [Draw(count: 1)]
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                counter,
                target,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("authored target-program fixture registry validates"),
    ));
    let decks = Some(vec![
        deck_with("island", &["authored_program_counter_target"]),
        deck_with("forest", &["authored_program_counter_then_draw"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_432, &[0, 1], 20, decks, true)
        .expect("authored target-program fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "authored_program_counter_target");
    inject_card_into_hand(&mut engine, 1, "authored_program_counter_then_draw");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "authored_program_counter_target"),
                Vec::new(),
            ),
        )
        .expect("cast the spell that will be targeted");
    let target_spell = engine.state.stack.last().expect("target spell on stack").id;

    let counter_slot = hand_index_for_card(&engine, 1, "authored_program_counter_then_draw") as u32;
    let legal = engine
        .apply_scenario_command(0, &pass())
        .expect("publish the opponent's target candidates");
    assert!(
        legal.legal_by_player[&1].valid_targets_by_hand_slot[&(counter_slot << 8)].groups[0]
            .valid_stack_ids
            .contains(&target_spell)
    );

    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                counter_slot as usize,
                vec![TargetRef {
                    object_id: target_spell,
                    kind: TargetRefKind::Stack as i32,
                    group_index: 0,
                    ..Default::default()
                }],
            ),
        )
        .expect("submit the exact authored program target reference");
    engine.state.players[0].mana_pool.colorless = 1;
    let draws_before = engine.state.turn_history.current.player(1).cards_drawn;
    pass_priority_round(&mut engine);

    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("soft-counter instruction parks for its target controller");
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::ManaPayment);
    submit_mana_resolution_decision(
        &mut engine,
        0,
        tricerules_proto::ruled::v1::ResolutionChoiceDecision::PayMana,
    )
    .expect("pay to preserve the target spell and resume the owner-bound body");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == target_spell));
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        draws_before + 1,
        "the instruction after the suspended target action runs exactly once"
    );
}

#[test]
fn authored_program_targets_publish_and_submit_for_activated_and_triggered_owners() {
    let owner = r#"(
        id: "authored_program_ability_target_source",
        name: "Authored Program Ability Target Source",
        face_id: "authored_program_ability_target_source",
        mana_cost: "{0}",
        types: ["Creature"],
        power: 2,
        toughness: 2,
        activated_abilities: [(
            ability_id: "activated_01", presentation: Fallback,
            source_zone: Battlefield, costs: [], effect: [],
            program: Some((body: (steps: [
                (id: "damage", kind: Perform(DamageTarget(amount: 1,
                    target: (kind: Creature, controller: Opponent))))
            ], yields: []), target_bindings: [
                (instruction_id: "damage", role: Target, target_group_index: 0),
            ])),
            targeting: Some((groups: [(min: 1, max: 1,
                prompt: "Choose an opposing creature", effect_indices: [])])),
            timing: Normal, conditions: [], activation_limit: None,
        )],
        triggered_abilities: [(
            ability_id: "triggered_01", presentation: Fallback,
            trigger: WhenSelfEntersBattlefield, effect: [],
            program: Some((body: (steps: [
                (id: "damage", kind: Perform(DamageTarget(amount: 1,
                    target: (kind: Creature, controller: Opponent))))
            ], yields: []), target_bindings: [
                (instruction_id: "damage", role: Target, target_group_index: 0),
            ])),
            modal: None,
            targeting: Some((groups: [(min: 1, max: 1,
                prompt: "Choose an opposing creature", effect_indices: [])])),
            may: false, intervening_if: None,
            triggers_only_once: false, max_triggers_per_turn: None,
        )]
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                owner,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("activated and triggered target programs compile for their exact owners"),
    ));
    let decks = Some(vec![
        deck_with(
            "forest",
            &["authored_program_ability_target_source", "forest", "forest"],
        ),
        deck_with("forest", &["grizzly_bears", "forest", "forest"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_433, &[0, 1], 20, decks, true)
        .expect("authored ability target fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let opposing_creature = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    inject_card_into_hand(&mut engine, 0, "authored_program_ability_target_source");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "authored_program_ability_target_source"),
                Vec::new(),
            ),
        )
        .expect("cast the permanent with a targeted ETB program");
    pass_priority_round(&mut engine);

    let source = *engine.state.players[0]
        .battlefield
        .iter()
        .find(|oid| engine.state.objects[oid].card_id == "authored_program_ability_target_source")
        .expect("the source resolved onto the battlefield");
    let ability_key = u64::from(source) << 32;
    let trigger_targets = &engine.initial_response_batch().legal_by_player[&0]
        .valid_targets_by_ability[&ability_key]
        .groups[0];
    assert_eq!(
        trigger_targets.valid_permanent_ids,
        vec![opposing_creature],
        "the pending trigger publishes candidates from its authored Target binding"
    );
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(
                    tricerules_proto::ruled::v1::ruled_command::Cmd::ChooseTriggerTarget(
                        tricerules_proto::ruled::v1::ChooseTriggerTarget {
                            decline: false,
                            selected_modes: Vec::new(),
                            targets: target_object(opposing_creature),
                        },
                    ),
                ),
            },
        )
        .expect("submit the exact authored target for the ETB ability");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&opposing_creature].damage, 1);

    let activation_targets = &engine.initial_response_batch().legal_by_player[&0]
        .valid_targets_by_ability[&ability_key]
        .groups[0];
    assert_eq!(
        activation_targets.valid_permanent_ids,
        vec![opposing_creature],
        "the printed activated ability publishes candidates through its own owner"
    );
    engine
        .apply_scenario_command(
            0,
            &activate_ability_for(&engine, source, 0, target_object(opposing_creature)),
        )
        .expect("submit the exact authored target for the activated ability");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&opposing_creature].zone,
        tricerules_core::Zone::Graveyard,
        "the two one-damage program owners resolve the selected 2/2 target"
    );
}

#[test]
fn authored_program_reuses_resolution_start_target_legality_after_tapping_target() {
    let source = r#"(
        id: "authored_program_tap_then_stun_source",
        name: "Authored Program Tap Then Stun Source",
        face_id: "authored_program_tap_then_stun_source",
        mana_cost: "{0}",
        types: ["Creature"],
        power: 2,
        toughness: 2,
        activated_abilities: [(
            ability_id: "activated_01", presentation: Fallback,
            source_zone: Battlefield, costs: [], effect: [],
            program: Some((body: (steps: [
                (id: "tap", kind: Perform(Tap(subject: Chosen((
                    kind: Creature, controller: Opponent, tapped: Some(false),
                ))))),
                (id: "stun", kind: Perform(PutCounters(
                    counter: Stun, count: 2, subject: Chosen((
                        kind: Creature, controller: Opponent, tapped: Some(false),
                    )),
                ))),
            ], yields: []), target_bindings: [
                (instruction_id: "tap", role: Target, target_group_index: 0),
                (instruction_id: "stun", role: Target, target_group_index: 0),
            ])),
            targeting: Some((groups: [(min: 1, max: 1,
                prompt: "Choose an untapped opposing creature", effect_indices: [])])),
            timing: Normal, conditions: [], activation_limit: None,
        )]
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                source,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("tap-then-stun fixture compiles against one shared target group"),
    ));
    let decks = Some(vec![
        deck_with(
            "forest",
            &["authored_program_tap_then_stun_source", "forest", "forest"],
        ),
        deck_with("forest", &["grizzly_bears", "forest", "forest"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_434, &[0, 1], 20, decks, true)
        .expect("tap-then-stun fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let opposing_creature = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let source = relocate_to_battlefield(
        &mut engine,
        0,
        "authored_program_tap_then_stun_source",
        false,
    );

    let ability_key = u64::from(source) << 32;
    assert_eq!(
        engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&ability_key]
            .groups[0]
            .valid_permanent_ids,
        vec![opposing_creature]
    );
    engine
        .apply_scenario_command(
            0,
            &activate_ability_for(&engine, source, 0, target_object(opposing_creature)),
        )
        .expect("submit the initially legal shared target");
    pass_priority_round(&mut engine);

    assert!(engine.state.objects[&opposing_creature].tapped);
    assert_eq!(
        engine.state.objects[&opposing_creature]
            .counters
            .get(&tricerules_card_model::CounterKind::Stun),
        Some(&2),
        "the shared mandatory target remains legal from resolution start after Tap, and the widened fixed count places both stun counters"
    );
}

#[test]
fn authored_program_static_grant_uses_its_exact_nested_ability_owner() {
    let grantor = r#"(
        id: "authored_program_grant_source",
        name: "Authored Program Grant Source",
        face_id: "authored_program_grant_source",
        mana_cost: "{0}",
        types: ["Artifact"],
        static_abilities: [(
            ability_id: "static_01", presentation: Fallback,
            definition: GrantActivatedAbilityToPermanents(
                filter: (kind: AnyPermanent, controller: You, permanent_types: [Creature]),
                activated_abilities: [(
                    ability_id: "activated_01", presentation: Fallback,
                    costs: [Tap], effect: [],
                    program: Some((body: (steps: [
                        (id: "damage", kind: Perform(DamageTarget(amount: 1,
                            target: (kind: Creature, controller: Opponent))))
                    ], yields: []), target_bindings: [
                        (instruction_id: "damage", role: Target, target_group_index: 0),
                    ])),
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose an opposing creature", effect_indices: [])]))
                )],
            ),
        )]
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                grantor,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("static grant compiles its complete nested authored program owner"),
    ));
    let decks = Some(vec![
        deck_with("island", &["authored_program_grant_source"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_436, &[0, 1], 20, decks, true)
        .expect("authored static-grant target fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "authored_program_grant_source");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "authored_program_grant_source"),
                Vec::new(),
            ),
        )
        .expect("cast the static grant source through the engine");
    pass_priority_round(&mut engine);
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    let legal = engine.initial_response_batch();
    let ability_targets = &legal.legal_by_player[&0].valid_targets_by_ability;
    let available_keys = ability_targets.keys().copied().collect::<Vec<_>>();
    let (&ability_key, target_offer) = ability_targets
        .iter()
        .find(|(key, _)| (**key >> 32) as u32 == recipient)
        .unwrap_or_else(|| {
            panic!(
                "the granted ability publishes a target offer; available keys: {available_keys:?}"
            )
        });
    let ability_index = ability_key as u32;
    let targets = &target_offer.groups[0];
    assert_eq!(targets.valid_permanent_ids, vec![opposing_creature]);
    engine
        .apply_scenario_command(
            0,
            &activate_ability_for(
                &engine,
                recipient,
                ability_index,
                target_object(opposing_creature),
            ),
        )
        .expect("activate the static-granted authored program with its exact target");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&opposing_creature].damage, 1);
}

#[test]
fn nested_authored_if_announces_each_owner_local_target_group() {
    // Both authored groups are required when the spell is cast, even though only one nested
    // branch executes. The active leaf must receive only the target from its own group.
    let owner = r#"(
        id: "nested_authored_target_union",
        name: "Nested Authored Target Union",
        face_id: "nested_authored_target_union",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "outer", kind: If(
                condition: CardsInHand(players: Relative(Controller),
                    min: Some(0), max: Some(50)),
                yes: (steps: [
                    (id: "inner", kind: If(
                        condition: ControllerLibraryEmpty,
                        yes: (steps: [
                            (id: "player_damage", kind: Perform(DamageTarget(
                                amount: 1, target: (kind: OpponentPlayer))))
                        ], yields: []),
                        no: (steps: [
                            (id: "creature_damage", kind: Perform(DamageTarget(
                                amount: 1, target: (kind: Creature, controller: Opponent))))
                        ], yields: [])
                    ))
                ], yields: []),
                no: (steps: [], yields: [])
            ))
        ], yields: []), target_bindings: [
            (instruction_id: "creature_damage", role: Target, target_group_index: 0),
            (instruction_id: "player_damage", role: Target, target_group_index: 1),
        ])),
        targeting: Some((groups: [
            (min: 1, max: 1, prompt: "Choose target opposing creature", effect_indices: []),
            (min: 1, max: 1, prompt: "Choose target opponent", effect_indices: []),
        ]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                owner,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("nested target branches compile under one face program owner"),
    ));
    let decks = Some(vec![
        deck_with("island", &["nested_authored_target_union", "island"]),
        deck_with("forest", &["grizzly_bears", "forest"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_434, &[0, 1], 20, decks, true)
        .expect("nested authored target fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let opposing_creature = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    inject_card_into_hand(&mut engine, 0, "nested_authored_target_union");
    let slot = hand_index_for_card(&engine, 0, "nested_authored_target_union");

    let offer = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    assert_eq!(offer.groups.len(), 2);
    assert_eq!(offer.groups[0].valid_permanent_ids, vec![opposing_creature]);
    assert!(offer.groups[1].can_target_opponent);

    let mut targets = target_object(opposing_creature);
    let mut opponent_target = target_player(1);
    opponent_target[0].group_index = 1;
    targets.extend(opponent_target);
    engine
        .apply_scenario_command(0, &cast_spell(slot, targets))
        .expect("submit both branch-union target groups");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&opposing_creature].damage, 1);
    assert_eq!(
        engine.state.players[1].life, 20,
        "the nested ControllerLibraryEmpty no-arm receives only group 0; group 1 was still announced"
    )
}

#[test]
fn authored_copy_program_can_retarget_a_copied_program_with_current_generation() {
    let targeted = r#"(
        id: "program_damage_for_copy",
        name: "Program Damage for Copy",
        face_id: "program_damage_for_copy",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "damage", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage", role: Target, target_group_index: 0),
        ])),
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose target creature", effect_indices: [])]))
    )"#;
    let copier = r#"(
        id: "authored_copy_program",
        name: "Authored Copy Program",
        face_id: "authored_copy_program",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [CopyTargetSpell(
            count: 1, spell_filter: (card_type: Some(InstantOrSorcery)))],
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose target instant or sorcery spell", effect_indices: [0])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                targeted,
                copier,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/generated/h/hill_giant.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("legacy copy effect and authored-damage program owner compile"),
    ));
    let decks = Some(vec![
        deck_with("island", &["program_damage_for_copy"]),
        deck_with("forest", &["authored_copy_program"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_435, &[0, 1], 20, decks, true)
        .expect("authored program-copy fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let original_target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let retarget = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");

    fn dev_move(engine: &mut GameEngine, card_name: &str, zone: i32) {
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(tricerules_proto::ruled::v1::DevCommand {
                        target_player_id: 1,
                        dev: Some(tricerules_proto::ruled::v1::dev_command::Dev::MoveCard(
                            tricerules_proto::ruled::v1::DevMoveCard {
                                card_name: card_name.to_owned(),
                                zone,
                                ready: true,
                            },
                        )),
                    })),
                },
            )
            .expect("move the exact retarget candidate between zones");
    }

    engine.enable_dev_commands();
    let original_retarget_generation = engine
        .state
        .zone_change_generation
        .get(&retarget)
        .copied()
        .unwrap_or(0);
    dev_move(
        &mut engine,
        "Hill Giant",
        tricerules_proto::ruled::v1::DevZone::Hand as i32,
    );
    dev_move(
        &mut engine,
        "Hill Giant",
        tricerules_proto::ruled::v1::DevZone::Battlefield as i32,
    );
    let retarget_generation = engine
        .state
        .zone_change_generation
        .get(&retarget)
        .copied()
        .unwrap_or(0);
    assert!(retarget_generation > original_retarget_generation);

    inject_card_into_hand(&mut engine, 0, "program_damage_for_copy");
    inject_card_into_hand(&mut engine, 1, "authored_copy_program");
    let target_slot = hand_index_for_card(&engine, 0, "program_damage_for_copy");
    let copy_slot = hand_index_for_card(&engine, 1, "authored_copy_program");
    engine
        .apply_scenario_command(0, &cast_spell(target_slot, target_object(original_target)))
        .expect("cast the authored targeted damage spell");
    let target_spell = engine.state.stack.last().expect("target spell on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority to copy the spell");

    let offered = &engine.initial_response_batch().legal_by_player[&1].valid_targets_by_hand_slot
        [&((copy_slot as u32) << 8)];
    assert!(offered.groups[0].valid_stack_ids.contains(&target_spell));
    engine
        .apply_scenario_command(1, &cast_spell(copy_slot, target_object(target_spell)))
        .expect("cast the target spell copier");
    pass_priority_round(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copied spell asks for a new target");
    assert!(pending.presentation.candidates.contains(&retarget));
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![retarget]))
        .expect("retarget the copied authored spell at the current candidate generation");

    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy && item.card_id == "program_damage_for_copy")
        .expect("the retargeted copy is on the stack");
    assert_eq!(copy.targets[0].object_id, retarget);
    assert_eq!(copy.targets[0].group_index, 0);
    assert_eq!(
        copy.targets[0].zone_change_generation,
        Some(retarget_generation)
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&original_target].damage, 1);
    assert_eq!(engine.state.objects[&retarget].damage, 1);
}

#[test]
fn authored_copy_keeps_stale_same_object_receipts_unless_current_incarnation_is_selected() {
    let targeted = r#"(
        id: "program_damage_for_same_object_copy",
        name: "Program Damage for Same Object Copy",
        face_id: "program_damage_for_same_object_copy",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "damage", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage", role: Target, target_group_index: 0),
        ])),
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose target creature", effect_indices: [])]))
    )"#;
    let copier = r#"(
        id: "legacy_copy_for_same_object",
        name: "Legacy Copy for Same Object",
        face_id: "legacy_copy_for_same_object",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [CopyTargetSpell(
            count: 1, spell_filter: (card_type: Some(InstantOrSorcery)))],
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose target instant or sorcery spell", effect_indices: [0])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                targeted,
                copier,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("authored damage program and legacy copier compile"),
    ));

    fn dev_move(engine: &mut GameEngine, zone: i32) {
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(tricerules_proto::ruled::v1::DevCommand {
                        target_player_id: 1,
                        dev: Some(tricerules_proto::ruled::v1::dev_command::Dev::MoveCard(
                            tricerules_proto::ruled::v1::DevMoveCard {
                                card_name: "Grizzly Bears".to_owned(),
                                zone,
                                ready: true,
                            },
                        )),
                    })),
                },
            )
            .expect("move the original target out and back before the copy resolves");
    }

    for (seed, choose_current_incarnation) in [(9_438, false), (9_439, true)] {
        let decks = Some(vec![
            deck_with("island", &["program_damage_for_same_object_copy"]),
            deck_with("forest", &["legacy_copy_for_same_object"]),
        ]);
        let mut engine = GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
            .expect("same-object copied-target fixture engine");
        advance_to_main1_from_game_start(&mut engine);
        let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        inject_card_into_hand(&mut engine, 0, "program_damage_for_same_object_copy");
        inject_card_into_hand(&mut engine, 1, "legacy_copy_for_same_object");
        let target_slot = hand_index_for_card(&engine, 0, "program_damage_for_same_object_copy");
        let copy_slot = hand_index_for_card(&engine, 1, "legacy_copy_for_same_object");
        engine.enable_dev_commands();

        engine
            .apply_scenario_command(0, &cast_spell(target_slot, target_object(target)))
            .expect("cast the authored spell at the physical target");
        let original_generation = engine
            .state
            .stack
            .last()
            .expect("authored spell on stack")
            .targets[0]
            .zone_change_generation
            .expect("object target receipt captures its generation");
        let target_spell = engine.state.stack.last().expect("target spell on stack").id;
        engine
            .apply_scenario_command(0, &pass())
            .expect("give the opponent priority to copy the spell");
        engine
            .apply_scenario_command(1, &cast_spell(copy_slot, target_object(target_spell)))
            .expect("cast the legacy target copier");

        dev_move(
            &mut engine,
            tricerules_proto::ruled::v1::DevZone::Hand as i32,
        );
        dev_move(
            &mut engine,
            tricerules_proto::ruled::v1::DevZone::Battlefield as i32,
        );
        let current_generation = engine
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .expect("returned permanent retains its new generation");
        assert!(current_generation > original_generation);

        pass_priority_round(&mut engine);
        assert!(engine.state.pending_resolution.is_some());
        if choose_current_incarnation {
            engine
                .apply_scenario_command(1, &submit_resolution_choice(vec![target]))
                .expect("choosing the returned object binds the copy to its current generation");
        } else {
            engine
                .apply_scenario_command(1, &submit_resolution_choice(Vec::new()))
                .expect("empty selection keeps the original generation-bound receipt");
        }

        let copy = engine
            .state
            .stack
            .iter()
            .find(|item| item.is_copy && item.card_id == "program_damage_for_same_object_copy")
            .expect("copy is pushed after its one occurrence is answered");
        assert_eq!(copy.targets[0].object_id, target);
        assert_eq!(
            copy.targets[0].zone_change_generation,
            Some(if choose_current_incarnation {
                current_generation
            } else {
                original_generation
            })
        );

        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&target].damage,
            u32::from(choose_current_incarnation),
            "only an explicitly selected current incarnation can be damaged"
        );
    }
}

#[test]
fn authored_copy_target_occurrences_keep_typed_domains_and_allow_unrelated_group_duplicates() {
    let targeted = r#"(
        id: "program_three_any_targets",
        name: "Program Three Any Targets",
        face_id: "program_three_any_targets",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "damage_one", kind: Perform(DamageTarget(
                amount: 1, target: (kind: AnyTarget)))),
            (id: "damage_two", kind: Perform(DamageTarget(
                amount: 1, target: (kind: AnyTarget)))),
            (id: "damage_three", kind: Perform(DamageTarget(
                amount: 1, target: (kind: AnyTarget))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage_one", role: Target, target_group_index: 0),
            (instruction_id: "damage_two", role: Target, target_group_index: 1),
            (instruction_id: "damage_three", role: Target, target_group_index: 2),
        ])),
        targeting: Some((groups: [
            (min: 1, max: 1, prompt: "First target", effect_indices: []),
            (min: 1, max: 1, prompt: "Second target", effect_indices: []),
            (min: 1, max: 1, prompt: "Third target", effect_indices: []),
        ]))
    )"#;
    let copier = r#"(
        id: "legacy_copy_for_three_targets",
        name: "Legacy Copy for Three Targets",
        face_id: "legacy_copy_for_three_targets",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [CopyTargetSpell(
            count: 1, spell_filter: (card_type: Some(InstantOrSorcery)))],
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose target instant or sorcery", effect_indices: [0])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                targeted,
                copier,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/generated/h/hill_giant.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("three authored target groups and the legacy copier compile"),
    ));
    let decks = Some(vec![
        deck_with("island", &["program_three_any_targets"]),
        deck_with("forest", &["legacy_copy_for_three_targets"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_437, &[0, 1], 20, decks, true)
        .expect("typed authored copy fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let first_permanent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second_permanent = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    inject_card_into_hand(&mut engine, 0, "program_three_any_targets");
    inject_card_into_hand(&mut engine, 1, "legacy_copy_for_three_targets");
    let target_slot = hand_index_for_card(&engine, 0, "program_three_any_targets");
    let copy_slot = hand_index_for_card(&engine, 1, "legacy_copy_for_three_targets");

    let mut original_targets = target_player(1);
    let mut second_target = target_object(first_permanent);
    second_target[0].group_index = 1;
    let mut third_target = target_object(second_permanent);
    third_target[0].group_index = 2;
    original_targets.extend(second_target);
    original_targets.extend(third_target);
    engine
        .apply_scenario_command(0, &cast_spell(target_slot, original_targets))
        .expect("cast the authored three-target spell");
    let target_spell = engine.state.stack.last().expect("target spell on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent a chance to copy the spell");
    engine
        .apply_scenario_command(1, &cast_spell(copy_slot, target_object(target_spell)))
        .expect("cast a legacy copy effect against the authored-program spell");

    let mut resolving_batch = None;
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority until the copier resolves");
        if batch.events.iter().any(|event| {
            matches!(
                event.ev,
                Some(Ev::ResolutionChoiceRequired(ref choice))
                    if choice.choice_kind == ChoiceKind::TargetObjects as i32
            )
        }) {
            resolving_batch = Some(batch);
        }
    }
    let batch = resolving_batch.expect("copy target prompt was published");
    let first_choice = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .expect("one target occurrence is presented");
    assert_eq!(
        first_choice.min, 0,
        "empty selection keeps this original target"
    );
    assert_eq!(
        first_choice.max, 1,
        "one typed target occurrence is asked at a time"
    );
    assert!(first_choice.prompt_text.contains("Target 1 of 3"));
    assert!(first_choice.prompt_text.contains("keep the original"));
    assert!(first_choice.candidate_object_ids.contains(&first_permanent));
    assert!(first_choice.candidate_player_ids.contains(&1));

    let submit_object = |object_id| RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: vec![object_id],
            ..Default::default()
        })),
    };
    let submit_player = |player_id| RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_player_ids: vec![player_id],
            ..Default::default()
        })),
    };
    assert!(engine
        .apply_scenario_command(1, &submit_object(1))
        .expect_err("a player id cannot be submitted in the object identity domain")
        .to_string()
        .contains("not offered"));
    engine
        .apply_scenario_command(1, &submit_player(first_permanent as i32))
        .expect("an object id in the player field leaves the occurrence prompt pending");
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.presentation.prompt.contains("Target 1 of 3")));
    engine
        .apply_scenario_command(1, &submit_object(first_permanent))
        .expect("replace the original player target with a permanent target");
    let second_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the second target occurrence is pending");
    assert_eq!(second_choice.presentation.min, 0);
    assert_eq!(second_choice.presentation.max, 1);
    engine
        .apply_scenario_command(1, &submit_player(1))
        .expect("replace the original permanent target with a player target");
    engine
        .apply_scenario_command(1, &submit_object(first_permanent))
        .expect("reuse a target in an unrelated group");

    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy && item.card_id == "program_three_any_targets")
        .expect("the copy is pushed only after all three occurrences are answered");
    assert_eq!(copy.targets.len(), 3);
    assert_eq!(copy.targets[0].object_id, first_permanent);
    assert_eq!(copy.targets[0].kind, TargetRefKind::Permanent as i32);
    assert_eq!(copy.targets[0].group_index, 0);
    assert_eq!(copy.targets[1].object_id, 1);
    assert_eq!(copy.targets[1].kind, TargetRefKind::Player as i32);
    assert_eq!(copy.targets[1].group_index, 1);
    assert_eq!(copy.targets[2].object_id, first_permanent);
    assert_eq!(copy.targets[2].kind, TargetRefKind::Permanent as i32);
    assert_eq!(copy.targets[2].group_index, 2);
}

#[test]
fn authored_copy_reprompts_when_final_targets_violate_group_uniqueness_or_distinctness() {
    let targeted = r#"(
        id: "program_distinct_copy_targets",
        name: "Program Distinct Copy Targets",
        face_id: "program_distinct_copy_targets",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "damage_one", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature)))),
            (id: "damage_two", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature)))),
            (id: "damage_three", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage_one", role: Target, target_group_index: 0),
            (instruction_id: "damage_two", role: Target, target_group_index: 0),
            (instruction_id: "damage_three", role: Target, target_group_index: 1),
        ])),
        targeting: Some((groups: [
            (min: 2, max: 2, prompt: "Choose two distinct creatures", effect_indices: [], distinct_from: [1]),
            (min: 1, max: 1, prompt: "Choose another creature", effect_indices: [], distinct_from: [0]),
        ]))
    )"#;
    let copier = r#"(
        id: "legacy_copy_for_distinct_targets",
        name: "Legacy Copy for Distinct Targets",
        face_id: "legacy_copy_for_distinct_targets",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [CopyTargetSpell(
            count: 1, spell_filter: (card_type: Some(InstantOrSorcery)))],
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose target instant or sorcery", effect_indices: [0])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                targeted,
                copier,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/generated/h/hill_giant.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("authored repeated target groups and legacy copy effect compile"),
    ));
    let decks = Some(vec![
        deck_with("island", &["program_distinct_copy_targets"]),
        deck_with("forest", &["legacy_copy_for_distinct_targets"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_440, &[0, 1], 20, decks, true)
        .expect("distinct copied-target fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let third = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "program_distinct_copy_targets");
    inject_card_into_hand(&mut engine, 1, "legacy_copy_for_distinct_targets");
    let target_slot = hand_index_for_card(&engine, 0, "program_distinct_copy_targets");
    let copy_slot = hand_index_for_card(&engine, 1, "legacy_copy_for_distinct_targets");
    let mut target_receipts = target_object(first);
    target_receipts.extend(target_object(second));
    let mut third_group = target_object(third);
    third_group[0].group_index = 1;
    target_receipts.extend(third_group);
    engine
        .apply_scenario_command(0, &cast_spell(target_slot, target_receipts))
        .expect("cast the authored spell with three distinct original targets");
    let target_spell = engine.state.stack.last().expect("target spell on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent a chance to copy the spell");
    engine
        .apply_scenario_command(1, &cast_spell(copy_slot, target_object(target_spell)))
        .expect("cast the legacy copy effect");
    pass_priority_round(&mut engine);

    let answer = |object_id| submit_resolution_choice(vec![object_id]);
    let answer_sequence = |engine: &mut GameEngine, targets: [u32; 3]| {
        for target in targets {
            engine
                .apply_scenario_command(1, &answer(target))
                .expect("each target occurrence is answered");
        }
    };
    answer_sequence(&mut engine, [first, first, third]);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| { pending.presentation.prompt.contains("Target 1 of 3") }));
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.is_copy && item.card_id == "program_distinct_copy_targets"));

    answer_sequence(&mut engine, [first, second, first]);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| { pending.presentation.prompt.contains("Target 1 of 3") }));
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.is_copy && item.card_id == "program_distinct_copy_targets"));

    // The first occurrence temporarily matches the unchanged second receipt; the final swap is
    // nevertheless valid once the second occurrence is answered, as CR 115.7e evaluates the set.
    answer_sequence(&mut engine, [second, first, third]);
    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy && item.card_id == "program_distinct_copy_targets")
        .expect("copy is pushed after a valid final target vector");
    assert_eq!(
        copy.targets
            .iter()
            .map(|target| (target.object_id, target.group_index))
            .collect::<Vec<_>>(),
        vec![(second, 0), (first, 0), (third, 1)]
    );
}

#[test]
fn synthetic_legendary_sacrifice_program_finishes_search_and_tail_before_903_9a() {
    // This test-only Legendary fixture is a legal commander type; Servant of the Stinger is not
    // legendary and cannot stand in for a commander when checking the post-resolution SBA.
    let fixture = r#"(
        id: "synthetic_legendary_sacrifice_program_commander",
        name: "Synthetic Legendary Sacrifice Program Commander",
        face_id: "synthetic_legendary_sacrifice_program_commander",
        mana_cost: "{0}",
        types: ["Legendary", "Creature", "Human"],
        power: 1,
        toughness: 1,
        triggered_abilities: [(
            ability_id: "triggered_01",
            presentation: Fallback,
            trigger: WhenSelfEntersBattlefield,
            effect: [],
            program: Some((body: (steps: [
                (id: "choose_sacrifice", kind: Choose(
                    choice: "sacrifice",
                    chooser: Controller,
                    optional: true,
                    branches: [(
                        branch_id: "sacrifice_commander",
                        presentation: Fallback,
                        cost: SacrificePermanent(
                            filter: (kind: AnyPermanent, controller: You),
                            source_only: true,
                        ),
                        requirement: Always,
                        body: (steps: [
                            (id: "search_library", kind: Perform(SearchLibrary(
                                filter: None,
                                destination: Hand,
                                shuffle: true,
                                reveal: false,
                                selection_bounds: Some((min: 1, max: 1)),
                            ))),
                        ], yields: []),
                    )],
                    otherwise: (steps: [], yields: []),
                )),
                (id: "parent_tail", kind: Perform(GainLife(amount: 3))),
            ], yields: []))),
        )],
    )"#;
    let card_id = "synthetic_legendary_sacrifice_program_commander";
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                fixture,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
            ],
            &[],
        )
        .expect("the synthetic Legendary sacrifice Program fixture compiles"),
    ));
    let decks = Some(vec![
        deck_with("forest", &[card_id]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(registry, 2026_1018, &[0, 1], 20, decks, true)
        .expect("two-player synthetic Legendary Program engine");
    advance_to_main1_from_game_start(&mut e);
    inject_card_into_hand(&mut e, 0, card_id);
    let hand_index = hand_index_for_card(&e, 0, card_id);
    e.apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast the zero-cost Legendary fixture");
    pass_both_players(&mut e);
    let commander = *e.state.players[0]
        .battlefield
        .iter()
        .find(|object_id| e.state.objects[object_id].card_id == card_id)
        .expect("the Legendary fixture resolved to the battlefield");
    e.state.players[0]
        .declared_commander_object_ids
        .push(commander);

    e.apply_scenario_command(0, &pass())
        .expect("P0 passes for the fixture trigger");
    let trigger_offer = e
        .apply_scenario_command(1, &pass())
        .expect("the fixture trigger offers its Program sacrifice branch");
    let choice = find_resolution_choice(&trigger_offer).expect("optional sacrifice branch");
    assert_eq!(choice.deciding_player_id, 0);
    let branch_prompt_id = e
        .state
        .pending_resolution
        .as_ref()
        .expect("branch choice is pending")
        .resolution_prompt_id
        .get();
    let mut choose = submit_resolution_choice(Vec::new());
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
        choose.cmd.as_mut()
    else {
        unreachable!()
    };
    answer.resolution_prompt_id = branch_prompt_id;
    answer.decision = tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32;
    answer.selected_branch_index = 0;
    e.apply_scenario_command(0, &choose)
        .expect("choose the Program's source-sacrifice branch");

    let payment_prompt_id = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the commander is offered as the sacrifice cost")
        .resolution_prompt_id
        .get();
    let paid = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander]))
        .expect("pay the source-only sacrifice cost");
    assert_eq!(
        e.state.objects[&commander].zone,
        tricerules_core::Zone::Graveyard
    );
    let search_prompt = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the branch body searches before commander SBA");
    assert_eq!(
        search_prompt.presentation.choice_kind,
        ChoiceKind::LibrarySearch
    );
    assert_eq!(
        (
            search_prompt.presentation.min,
            search_prompt.presentation.max
        ),
        (1, 1)
    );
    let search_prompt_id = search_prompt.resolution_prompt_id.get();
    let searched_card = search_prompt.presentation.candidates[0];
    let starting_life = e.state.players[0].life;
    let searched = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![searched_card]))
        .expect("finish the branch search and its parent tail");

    assert_eq!(e.state.players[0].life, starting_life + 3);
    assert!(e.state.players[0].hand.contains(&searched_card));
    assert_eq!(
        e.state.objects[&commander].zone,
        tricerules_core::Zone::Graveyard
    );
    let commander_sba = find_resolution_choice(&searched)
        .expect("only after the search and parent tail does CR 903.9a ask the owner");
    assert_eq!(commander_sba.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(commander_sba.deciding_player_id, 0);
    let commander_prompt_id = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the commander SBA choice is pending")
        .resolution_prompt_id
        .get();
    let life_event_index = searched
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::LifeChanged(change))
                    if change.delta == 3
            )
        })
        .expect("the Program parent tail completed");
    let searched_card_event_index = searched
        .events
        .iter()
        .position(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == searched_card
                    && moved.destination
                        == tricerules_proto::ruled::v1::permanent_moved::Destination::Hand as i32
        ))
        .expect("the library-search result reached the hand before the tail");
    let shuffle_event_index = searched
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text == "P0 shuffles their library."
            )
        })
        .expect("the search shuffle completes before the tail");
    let sba_event_index = searched
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                    if choice.choice_kind == ChoiceKind::ResolutionBranch as i32
                        && choice.deciding_player_id == 0
            )
        })
        .expect("the commander SBA choice is presented after resolution");
    assert!(searched_card_event_index < life_event_index);
    assert!(shuffle_event_index < life_event_index);
    assert!(life_event_index < sba_event_index);

    let mut leave_in_graveyard = submit_resolution_choice(Vec::new());
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
        leave_in_graveyard.cmd.as_mut()
    else {
        unreachable!()
    };
    answer.resolution_prompt_id = commander_prompt_id;
    answer.decision = tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32;
    answer.selected_branch_index = 1;
    e.apply_scenario_command(0, &leave_in_graveyard)
        .expect("leave the commander in its graveyard");
    assert_eq!(
        e.state.objects[&commander].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.state.pending_resolution.is_none());

    let batches = [&paid, &searched];
    assert_eq!(
        batches
            .iter()
            .flat_map(|batch| &batch.events)
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text == "P0 shuffles their library."
            ))
            .count(),
        1,
        "the Program search completes once before the SBA choice"
    );
    assert_ne!(payment_prompt_id, search_prompt_id);
}

#[test]
fn authored_soft_counter_resumes_at_its_following_discard_after_declined_payment() {
    let counter = r#"(
        id: "program_counter_then_loot_decline",
        name: "Program Counter Then Loot Decline",
        face_id: "program_counter_then_loot_decline",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "counter", kind: Perform(CounterTargetSpell(
                spell_filter: (), unless_controller_pays: Some(1)
            ))),
            (id: "draw", kind: Perform(Draw(who: Controller, count: 1))),
            (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                who: Controller, quantity: Exact(1)
            )))
        ], yields: []), target_bindings: [
            (instruction_id: "counter", role: Spell, target_group_index: 0)
        ])),
        targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target spell",
            effect_indices: [])]))
    )"#;
    let target = r#"(
        id: "program_counter_decline_target",
        name: "Program Counter Decline Target",
        face_id: "program_counter_decline_target",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [Draw(count: 1)]
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                counter,
                target,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("soft-counter decline fixture registry validates"),
    ));
    let decks = Some(vec![
        deck_with("island", &["program_counter_decline_target"]),
        deck_with("forest", &["program_counter_then_loot_decline"]),
    ]);
    let mut engine = GameEngine::new(registry, 9_432, &[0, 1], 20, decks, true)
        .expect("soft-counter decline fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "program_counter_decline_target");
    inject_card_into_hand(&mut engine, 1, "program_counter_then_loot_decline");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "program_counter_decline_target"),
                Vec::new(),
            ),
        )
        .expect("cast the spell that will be targeted");
    let target_spell = engine.state.stack.last().expect("target spell on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("give the opponent priority to respond");
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "program_counter_then_loot_decline"),
                vec![TargetRef {
                    object_id: target_spell,
                    kind: TargetRefKind::Stack as i32,
                    group_index: 0,
                    ..Default::default()
                }],
            ),
        )
        .expect("cast the source-lowered soft counter in response");
    let draws_before = engine.state.turn_history.current.player(1).cards_drawn;
    pass_priority_round(&mut engine);

    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the target controller may decline the soft-counter cost");
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::ManaPayment);
    assert_eq!(payment.deciding_player, 0);
    submit_mana_resolution_decision(
        &mut engine,
        0,
        tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
    )
    .expect("decline to preserve the target spell");

    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the authored discard after a declined payment must resume");
    assert_eq!(discard.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!(discard.deciding_player, 1);
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == target_spell));
    assert_eq!(
        engine.state.objects[&target_spell].zone,
        tricerules_core::Zone::Graveyard
    );
    let discarded = discard.presentation.candidates[0];
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("finish the resumed authored discard step");
    assert_eq!(
        engine.state.objects[&discarded].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        draws_before + 1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(
        engine.state.stack.is_empty(),
        "both spells have left the stack"
    );
}

#[test]
fn authored_copy_tokens_resume_after_entry_counter_replacement_and_order_choices() {
    let program = r#"(
        id: "copy_then_draw_program",
        name: "Copy Then Draw Program",
        face_id: "copy_then_draw_program",
        mana_cost: "{0}",
        types: ["Instant"],
        modal_spell: (
            min_modes: 1,
            max_modes: 1,
            modes: [
                (mode_id: "copy_mode", presentation: Fallback,
                    effects: [],
                    program: Some((body: (steps: [
                        (id: "draw_first", kind: Perform(Draw(who: Controller, count: 1))),
                        (id: "discard", kind: Perform(Discard(who: Controller,
                            quantity: Exact(1)))),
                        (id: "copy_creature", kind: Perform(CreateTokenCopies(count: 2,
                            source: Chosen((kind: Creature, controller: You))))),
                        (id: "draw_last", kind: Perform(Draw(who: Controller, count: 1)))
                    ], yields: []), target_bindings: [
                        (instruction_id: "copy_creature", role: Source,
                            target_group_index: 0)
                    ])),
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose target creature you control", effect_indices: [])])))
            ]
        )
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/orb_of_dreams.ron"),
                include_str!("../../../tricerules-cards/data/doubling_season.ron"),
                include_str!("../../../tricerules-cards/data/laezel,_vlaakiths_champion.ron"),
                include_str!("../../../tricerules-cards/data/pentavus.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/pentavite_c_1_1_flying.ron"
            )],
        )
        .expect("copy-token program registry validates"),
    ));
    let decks = Some(vec![vec!["island".into(); 30], vec!["forest".into(); 30]]);
    let mut engine = GameEngine::new(registry, 9_429, &[0, 1], 20, decks, true)
        .expect("copy-token program fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "pentavus");
    let source = move_ready_to_battlefield(&mut engine, 0, "pentavus");
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        5
    );
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "laezel,_vlaakiths_champion");
    inject_card_into_hand(&mut engine, 0, "copy_then_draw_program");
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(
                hand_index_for_card(&engine, 0, "copy_then_draw_program"),
                vec![(0, target_object(source))],
            ),
        )
        .expect("cast the authored token-copy program");
    pass_priority_round(&mut engine);
    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the authored hand choice resumes before token creation");
    assert_eq!(discard.presentation.choice_kind, ChoiceKind::HandCards);
    let discarded = discard.presentation.candidates[0];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("complete the authored draw step before token creation");
    let draws_before_entries = engine.state.turn_history.current.player(0).cards_drawn;
    assert_eq!(draws_before_entries, draws_before + 1);

    // Doubling Season doubles the two copied tokens. The resulting batch parks for each token's
    // tapped-entry and entry-counter replacements, then for simultaneous-entry order. Completing
    // each choice must retain the source program cursor.
    let mut entry_replacement_choices = 0;
    let mut counter_replacement_choices = 0;
    let mut entry_order_choices = 0;
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        let choice_kind = pending.presentation.choice_kind;
        let prompt = pending.presentation.prompt.clone();
        let candidates = pending.presentation.candidates.clone();
        match choice_kind {
            ChoiceKind::ReplacementEffect => {
                let replacement = *candidates
                    .first()
                    .expect("replacement choice has an application candidate");
                if prompt.contains("counter replacement") {
                    counter_replacement_choices += 1;
                    engine
                        .apply_scenario_command(0, &submit_resolution_choice(vec![replacement]))
                        .expect("choose one token entry-counter replacement");
                } else {
                    entry_replacement_choices += 1;
                    engine
                        .apply_scenario_command(0, &submit_resolution_choice(vec![replacement]))
                        .expect("choose one token-entry replacement");
                }
            }
            ChoiceKind::SimultaneousEntryOrder => {
                entry_order_choices += 1;
                engine
                    .apply_scenario_command(0, &submit_resolution_choice(candidates))
                    .expect("order the simultaneous copied-token entries");
            }
            other => panic!("unexpected token-copy continuation choice: {other:?}"),
        }
    }
    assert_eq!(entry_replacement_choices, 8);
    assert_eq!(counter_replacement_choices, 4);
    assert_eq!(entry_order_choices, 1);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before_entries + 1,
        "the authored program resumes after replacement and ordering choices"
    );
    let copied_tokens = engine
        .state
        .objects
        .values()
        .filter(|object| {
            object.is_token()
                && object.card_id == "pentavus"
                && object.zone == tricerules_core::Zone::Battlefield
        })
        .collect::<Vec<_>>();
    assert_eq!(copied_tokens.len(), 4);
    assert!(copied_tokens.iter().all(|object| object.tapped));
    assert!(copied_tokens.iter().all(|object| {
        matches!(
            object
                .counters
                .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
            Some(11 | 12)
        )
    }));
}

#[test]
fn source_lowered_recruit_finishes_after_its_token_entry_replacement_choice() {
    let decks = Some(vec![
        deck_with("plains", &["long_lake_nuisance", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        9_430,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Recruit source-lowering fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    let discarded = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|object| engine.state.objects[object].card_id == "grizzly_bears")
        .expect("Recruit's known nonland discard");
    move_ready_to_battlefield(&mut engine, 0, "long_lake_nuisance");
    // Add the replacement sources after Recruit enters so the fixture exercises only token-entry
    // replacement suspension, not the same replacements on the source permanent.
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    pass_both_players(&mut engine);

    let discard_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Recruit's source-lowered discard choice");
    assert_eq!(
        discard_choice.presentation.choice_kind,
        ChoiceKind::HandCards
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard the Recruit's nonland card");

    let mut replacement_choices = 0;
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ReplacementEffect
        );
        let replacement = pending.presentation.candidates[0];
        engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![replacement]))
            .expect("choose the token's Orb of Dreams replacement");
        replacement_choices += 1;
        assert!(
            replacement_choices <= 2,
            "unexpected repeated token entry choices"
        );
    }
    assert!(
        replacement_choices > 0,
        "the token entry replacement suspended resolution"
    );

    let soldiers = battlefield_token_oids(&engine, 0, "human_soldier_w_1_1");
    assert_eq!(soldiers.len(), 1);
    assert!(engine.state.objects[&soldiers[0]].tapped);
    assert_eq!(
        engine.state.objects[&discarded].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(
        engine.state.stack.is_empty(),
        "the triggered program exits once"
    );
}

#[test]
fn optional_exact_discard_through_library_of_leng_draws_after_hidden_replacement() {
    let program = r#"(
        id: "optional_exact_leng_program",
        name: "Optional Exact Leng Program",
        face_id: "optional_exact_leng_program",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "discard_one", bind: ["discard_result"], kind: Perform(
                    Discard(who: Controller, quantity: OptionalExact(1)))),
                (id: "draw_if_paid", kind: IfResult(
                    condition: DiscardOutcome(
                        result: "discard_result", outcome: ChosenToPay),
                    yes: (steps: [
                        (id: "paid_draw", kind: Perform(Draw(count: 1)))
                    ], yields: []),
                    no: (steps: [], yields: [])
                ))
            ],
            yields: []
        )))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/library_of_leng.ron"),
            ],
            &[],
        )
        .expect("optional exact Library of Leng fixture registry validates"),
    ));
    let decks = Some(vec![vec!["island".into(); 30], vec!["forest".into(); 30]]);
    let mut engine = GameEngine::new(registry, 9_428, &[0, 1], 20, decks, true)
        .expect("optional exact Leng fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let hidden_nonland = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "optional_exact_leng_program");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "optional_exact_leng_program"),
                Vec::new(),
            ),
        )
        .expect("cast the zero-cost optional exact Leng program");
    pass_priority_round(&mut engine);

    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("optional exact discard choice");
    assert_eq!((discard.presentation.min, discard.presentation.max), (0, 1));
    assert!(discard.presentation.candidates.contains(&hidden_nonland));
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&hidden_nonland)
        .copied()
        .unwrap_or(0);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![hidden_nonland]))
        .expect("choose the full optional payment");

    let replacement = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Library of Leng replacement choice");
    assert_eq!(
        replacement.presentation.choice_kind,
        ChoiceKind::PrivateReplacement
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![1]))
        .expect("put the paid discard on top of the library");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&hidden_nonland].zone,
        tricerules_core::Zone::Hand
    );
    assert!(engine.state.players[0].hand.contains(&hidden_nonland));
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&hidden_nonland)
            .copied(),
        Some(generation_before + 2),
        "the card moved to the library as a hidden discard, then the follow-up draw moved it back"
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "ChosenToPay remains true through an unknown hidden cohort, so the follow-up draw resolves"
    );
}

#[test]
fn discard_count_branch_counts_public_madness_exile_as_known_nonland() {
    let program = r#"(
        id: "known_madness_discard_program",
        name: "Known Madness Discard Program",
        face_id: "known_madness_discard_program",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "discard_one", bind: ["discard_result"], kind: Perform(
                    Discard(who: Controller, quantity: Exact(1)))),
                (id: "draw_if_known_nonland", kind: IfResult(
                    condition: DiscardCount(
                        result: "discard_result", players: Controller,
                        card_type: Some(Nonland), min: Some(1), max: None),
                    yes: (steps: [
                        (id: "known_nonland_draw", kind: Perform(Draw(count: 1)))
                    ], yields: []),
                    no: (steps: [], yields: [])
                ))
            ],
            yields: []
        )))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/fiery_temper.ron"),
                include_str!("../../../tricerules-cards/data/library_of_leng.ron"),
            ],
            &[],
        )
        .expect("known Madness discard fixture registry validates"),
    ));
    let decks = Some(vec![vec!["island".into(); 30], vec!["forest".into(); 30]]);
    let mut engine = GameEngine::new(registry, 9_429, &[0, 1], 20, decks, true)
        .expect("known Madness fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let madness_card = inject_card_into_hand(&mut engine, 0, "fiery_temper");
    inject_card_into_hand(&mut engine, 0, "known_madness_discard_program");
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "known_madness_discard_program"),
                Vec::new(),
            ),
        )
        .expect("cast the zero-cost program");
    pass_priority_round(&mut engine);

    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("mandatory discard choice");
    assert!(discard.presentation.candidates.contains(&madness_card));
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![madness_card]))
        .expect("choose the Madness card");
    let replacement = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Library of Leng replacement choice");
    assert_eq!(
        replacement.presentation.choice_kind,
        ChoiceKind::PrivateReplacement
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![2]))
        .expect("choose the public Madness exile destination");

    assert_eq!(
        engine.state.objects[&madness_card].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "a public Madness exile retains known nonland types for DiscardCount"
    );
}

#[test]
fn modal_authored_program_binds_each_modes_local_group_after_the_global_fizzle_check() {
    let modal_program = r#"(
        id: "modal_targeted_draw_discard_program",
        name: "Modal Targeted Draw Discard Program",
        face_id: "modal_targeted_draw_discard_program",
        mana_cost: "{0}",
        types: ["Instant"],
        modal_spell: (
            min_modes: 2,
            max_modes: 2,
            modes: [
                (mode_id: "first_mode", presentation: Fallback,
                    effects: [],
                    program: Some((body: (steps: [
                        (id: "damage", kind: Perform(DamageTarget(
                            amount: 1, target: (kind: Creature, controller: Opponent)))),
                        (id: "draw", kind: Perform(Draw(who: Controller, count: 1)))
                    ], yields: []), target_bindings: [
                        (instruction_id: "damage", role: Target, target_group_index: 0),
                    ])),
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose a creature for the first mode", effect_indices: [])]))),
                (mode_id: "second_mode", presentation: Fallback,
                    effects: [],
                    program: Some((body: (steps: [
                        (id: "damage", kind: Perform(DamageTarget(
                            amount: 1, target: (kind: Creature, controller: You)))),
                        (id: "draw", kind: Perform(Draw(who: Controller, count: 1)))
                    ], yields: []), target_bindings: [
                        (instruction_id: "damage", role: Target, target_group_index: 0),
                    ])),
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose a creature for the second mode", effect_indices: [])])))
            ]
        )
    )"#;
    let fatal_ping = r#"(
        id: "targeted_fatal_ping",
        name: "Targeted Fatal Ping",
        face_id: "targeted_fatal_ping",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [TargetPlayerLosesLife(amount: 1, target: (kind: AnyPlayer))],
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose a player", effect_indices: [0])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                modal_program,
                fatal_ping,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            ],
            &[],
        )
        .expect("modal authored target-program fixture registry validates"),
    ));
    let decks = Some(vec![
        vec!["island".into(); 30],
        vec!["forest".into(); 30],
        vec!["island".into(); 30],
    ]);
    let mut engine = GameEngine::new(registry, 9_428, &[0, 1, 2], 20, decks, true)
        .expect("three-player modal target fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    engine.state.players[1].life = 1;
    inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let first_mode_target = put_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second_mode_target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "modal_targeted_draw_discard_program");
    inject_card_into_hand(&mut engine, 2, "targeted_fatal_ping");
    let cards_drawn_before_resolution = engine.state.turn_history.current.player(0).cards_drawn;
    let modal_slot = hand_index_for_card(&engine, 0, "modal_targeted_draw_discard_program");
    let legal = engine.initial_response_batch();
    let modal_action = legal.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == modal_slot as u32)
        .expect("modal card has an authoritative cast offer");
    assert_eq!(
        modal_action.modes[0].targets.as_ref().unwrap().groups[0].valid_permanent_ids,
        vec![first_mode_target],
        "first mode targets its own local group 0"
    );
    assert_eq!(
        modal_action.modes[1].targets.as_ref().unwrap().groups[0].valid_permanent_ids,
        vec![second_mode_target],
        "second mode targets its own local group 0"
    );

    engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(
                modal_slot,
                vec![
                    (0, target_object(first_mode_target)),
                    (1, target_object(second_mode_target)),
                ],
            ),
        )
        .expect("cast both stable modes with owner-local player targets");
    engine
        .apply_scenario_command(0, &pass())
        .expect("the casting player passes priority");
    engine
        .apply_scenario_command(1, &pass())
        .expect("the first opponent passes priority");
    engine
        .apply_scenario_command(
            2,
            &cast_spell(
                hand_index_for_card(&engine, 2, "targeted_fatal_ping"),
                target_player(1),
            ),
        )
        .expect("the third player responds by eliminating the first mode's target");
    pass_priority_round(&mut engine);
    assert!(engine.state.players[1].has_lost);
    pass_priority_round(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert!(
        !engine.state.objects.contains_key(&first_mode_target),
        "the first mode's target left with its eliminated player"
    );
    assert_eq!(
        engine.state.objects[&second_mode_target].damage, 1,
        "the second mode retains its own target and continues after the one global fizzle check"
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        cards_drawn_before_resolution + 2,
        "each exact mode program continues after its own target check"
    );
}

#[test]
fn modal_authored_program_fizzles_once_when_all_mode_targets_are_illegal() {
    let modal_program = r#"(
        id: "all_illegal_modal_draw_discard_program",
        name: "All Illegal Modal Draw Discard Program",
        face_id: "all_illegal_modal_draw_discard_program",
        mana_cost: "{0}",
        types: ["Instant"],
        modal_spell: (
            min_modes: 2,
            max_modes: 2,
            modes: [
                (mode_id: "first_mode", presentation: Fallback,
                    effects: [],
                    program: Some((body: (steps: [
                        (id: "damage", kind: Perform(DamageTarget(
                            amount: 1, target: (kind: Creature)))),
                        (id: "draw", kind: Perform(Draw(who: Controller, count: 1)))
                    ], yields: []), target_bindings: [
                        (instruction_id: "damage", role: Target, target_group_index: 0),
                    ])),
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose a creature for the first mode", effect_indices: [])]))),
                (mode_id: "second_mode", presentation: Fallback,
                    effects: [],
                    program: Some((body: (steps: [
                        (id: "damage", kind: Perform(DamageTarget(
                            amount: 1, target: (kind: Creature)))),
                        (id: "draw", kind: Perform(Draw(who: Controller, count: 1)))
                    ], yields: []), target_bindings: [
                        (instruction_id: "damage", role: Target, target_group_index: 0),
                    ])),
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose a creature for the second mode", effect_indices: [])])))
            ]
        )
    )"#;
    let fatal_ping = r#"(
        id: "all_illegal_targeted_fatal_ping",
        name: "All Illegal Targeted Fatal Ping",
        face_id: "all_illegal_targeted_fatal_ping",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [TargetPlayerLosesLife(amount: 1, target: (kind: AnyPlayer))],
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose a player", effect_indices: [0])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                modal_program,
                fatal_ping,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            ],
            &[],
        )
        .expect("all-illegal modal target fixture registry validates"),
    ));
    let decks = Some(vec![
        vec!["island".into(); 30],
        vec!["forest".into(); 30],
        vec!["island".into(); 30],
    ]);
    let mut engine = GameEngine::new(registry, 9_429, &[0, 1, 2], 20, decks, true)
        .expect("all-illegal modal target fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    engine.state.players[1].life = 1;
    inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let first_mode_target = put_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second_mode_target = put_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "all_illegal_modal_draw_discard_program");
    inject_card_into_hand(&mut engine, 2, "all_illegal_targeted_fatal_ping");
    let cards_drawn_before_resolution = engine.state.turn_history.current.player(0).cards_drawn;
    let modal_card = engine.state.players[0].hand
        [hand_index_for_card(&engine, 0, "all_illegal_modal_draw_discard_program")];

    engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(
                hand_index_for_card(&engine, 0, "all_illegal_modal_draw_discard_program"),
                vec![
                    (0, target_object(first_mode_target)),
                    (1, target_object(second_mode_target)),
                ],
            ),
        )
        .expect("cast both modes with currently legal targets controlled by one player");
    engine
        .apply_scenario_command(0, &pass())
        .expect("the casting player passes priority");
    engine
        .apply_scenario_command(1, &pass())
        .expect("the first opponent passes priority");
    engine
        .apply_scenario_command(
            2,
            &cast_spell(
                hand_index_for_card(&engine, 2, "all_illegal_targeted_fatal_ping"),
                target_player(1),
            ),
        )
        .expect("the third player responds by eliminating both mode targets' controller");
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);

    assert!(engine.state.players[1].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        cards_drawn_before_resolution,
        "the stack-wide fizzle prevents every mode's draw-discard instruction"
    );
    assert_eq!(
        engine.state.objects[&modal_card].zone,
        tricerules_core::Zone::Graveyard,
        "a spell with all targets illegal leaves the stack without resolving its program"
    );
}

#[test]
fn resolution_answers_require_the_exact_active_prompt_before_any_mutation() {
    let decks = Some(vec![
        {
            let mut deck = vec!["brainstorm".to_string(), "brainstorm".to_string()];
            deck.extend(std::iter::repeat_n("island".to_string(), 28));
            deck
        },
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        42_001,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first_batch = cast_instant_and_resolve(
        &mut engine,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert!(find_resolution_choice(&first_batch).is_some());

    let first_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("first Brainstorm choice")
        .resolution_prompt_id
        .get();
    assert_ne!(first_id, 0);
    let chosen: Vec<u32> = engine.state.players[0]
        .hand
        .iter()
        .take(2)
        .copied()
        .collect();
    let mut missing_id = submit_resolution_choice(chosen.clone());
    let Some(Cmd::SubmitResolutionChoice(answer)) = missing_id.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = 0;
    let state_before_rejections = format!("{:?}", engine.state);
    let command_index_before_rejections = engine.state.command_index;
    assert!(
        engine.apply_command(0, &missing_id).is_err(),
        "missing ID rejected"
    );
    assert_eq!(format!("{:?}", engine.state), state_before_rejections);
    assert_eq!(engine.state.command_index, command_index_before_rejections);

    let mut foreign_actor = submit_resolution_choice(chosen.clone());
    let Some(Cmd::SubmitResolutionChoice(answer)) = foreign_actor.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = first_id;
    assert!(
        engine.apply_command(1, &foreign_actor).is_err(),
        "the correct ID from the wrong actor is rejected"
    );
    assert_eq!(format!("{:?}", engine.state), state_before_rejections);
    assert_eq!(engine.state.command_index, command_index_before_rejections);

    let mut stale_id = submit_resolution_choice(chosen.clone());
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_id.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = first_id + 99;
    assert!(
        engine.apply_command(0, &stale_id).is_err(),
        "stale ID rejected"
    );
    assert_eq!(format!("{:?}", engine.state), state_before_rejections);
    assert_eq!(engine.state.command_index, command_index_before_rejections);

    let stale_preview = engine.preview_payment(
        0,
        &tricerules_proto::ruled::v1::PreviewPayment {
            resolution_choice: match stale_id.cmd {
                Some(Cmd::SubmitResolutionChoice(answer)) => Some(answer),
                _ => unreachable!(),
            },
            ..Default::default()
        },
    );
    assert!(!stale_preview.valid, "stale resolution preview is rejected");
    assert_eq!(format!("{:?}", engine.state), state_before_rejections);
    assert_eq!(engine.state.command_index, command_index_before_rejections);

    let first_answer = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: first_id,
            chosen_object_ids: chosen,
            ..Default::default()
        })),
    };
    engine
        .apply_command(0, &first_answer)
        .expect("exact current ID accepts the answer");

    let second_batch = cast_instant_and_resolve(
        &mut engine,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert!(find_resolution_choice(&second_batch).is_some());
    let second_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("second Brainstorm choice")
        .resolution_prompt_id
        .get();
    assert!(
        second_id > first_id,
        "new logical offer receives a fresh ID"
    );
}

#[test]
fn identical_consecutive_authored_offers_get_fresh_ids_and_replay_from_wire() {
    let program = r#"(
        id: "identical_optional_choice_program",
        name: "Identical Optional Choice Program",
        face_id: "identical_optional_choice_program",
        mana_cost: "{U}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "first_choice", bind: ["first"], kind: Perform(ChoosePermanents(
                    chooser: Controller,
                    filter: (kind: AnyPermanent, permanent_types: [Land]),
                    min: 0,
                    max: 1,
                ))),
                (id: "second_choice", bind: ["second"], kind: Perform(ChoosePermanents(
                    chooser: Controller,
                    filter: (kind: AnyPermanent, permanent_types: [Land]),
                    min: 0,
                    max: 1,
                ))),
            ],
            yields: [],
        ))),
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("identical-choice fixture registry validates"),
    ));
    let make_prompt = || {
        let mut engine = GameEngine::new(
            registry,
            42_002,
            &[0, 1],
            20,
            Some(vec![vec!["island".into(); 30], vec!["forest".into(); 30]]),
            true,
        )
        .expect("new identical-choice fixture game");
        advance_to_main1_from_game_start(&mut engine);
        inject_permanent_on_battlefield(&mut engine, 0, "island");
        inject_card_into_hand(&mut engine, 0, "identical_optional_choice_program");
        let batch = cast_instant_and_resolve(
            &mut engine,
            0,
            "identical_optional_choice_program",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let choice = find_resolution_choice(&batch).expect("first optional choice");
        (engine, batch, choice)
    };

    let (mut original, original_batch, first_offer) = make_prompt();
    let (mut replay, replay_batch, replay_first_offer) = make_prompt();
    assert_eq!(original_batch, replay_batch);
    assert_eq!(first_offer, replay_first_offer);
    assert_eq!(first_offer.min, 0);
    assert_eq!(first_offer.max, 1);
    let first_id = first_offer.resolution_prompt_id;
    assert_ne!(first_id, 0);
    let first_state = original_batch
        .resolution_prompt_state
        .as_ref()
        .expect("first prompt has final state");
    assert!(first_state.active);
    assert_eq!(first_state.resolution_prompt_id, Some(first_id));

    let mut accepted_answer = submit_resolution_choice(Vec::new());
    let Some(Cmd::SubmitResolutionChoice(answer)) = accepted_answer.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = first_id;
    let serialized_command = accepted_answer.encode_to_vec();
    let decoded_command = RuledCommand::decode(serialized_command.as_slice())
        .expect("correlated answer decodes from the accepted command log");
    assert_eq!(decoded_command, accepted_answer);
    let original_second_batch = original
        .apply_command(0, &decoded_command)
        .expect("raw protobuf answer advances the first logical offer");
    let replay_second_batch = replay
        .apply_command(0, &decoded_command)
        .expect("same raw protobuf answer replays deterministically");
    assert_eq!(original_second_batch, replay_second_batch);
    let second_offer = find_resolution_choice(&original_second_batch)
        .expect("the identical second optional choice is installed");
    let replay_second_offer = find_resolution_choice(&replay_second_batch).unwrap();
    assert_eq!(second_offer, replay_second_offer);
    assert_ne!(second_offer.resolution_prompt_id, first_id);
    let second_state = original_second_batch
        .resolution_prompt_state
        .as_ref()
        .expect("second prompt has final state");
    assert!(second_state.active);
    assert_eq!(
        second_state.resolution_prompt_id,
        Some(second_offer.resolution_prompt_id)
    );
    let mut normalized_first = first_offer.clone();
    normalized_first.resolution_prompt_id = 0;
    let mut normalized_second = second_offer.clone();
    normalized_second.resolution_prompt_id = 0;
    assert_eq!(
        normalized_first, normalized_second,
        "the production path emits byte-identical answer semantics for both instructions"
    );

    let mut duplicate_answer = decoded_command.clone();
    let Some(Cmd::SubmitResolutionChoice(answer)) = duplicate_answer.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = first_id;
    let state_before_duplicate = format!("{:?}", original.state);
    let command_index_before_duplicate = original.state.command_index;
    assert!(original.apply_command(0, &duplicate_answer).is_err());
    assert_eq!(format!("{:?}", original.state), state_before_duplicate);
    assert_eq!(original.state.command_index, command_index_before_duplicate);

    let mut second_answer = submit_resolution_choice(Vec::new());
    let Some(Cmd::SubmitResolutionChoice(answer)) = second_answer.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = second_offer.resolution_prompt_id;
    let serialized_second = second_answer.encode_to_vec();
    let decoded_second = RuledCommand::decode(serialized_second.as_slice()).unwrap();
    let original_final_batch = original.apply_command(0, &decoded_second).unwrap();
    let replay_final_batch = replay.apply_command(0, &decoded_second).unwrap();
    assert_eq!(original_final_batch, replay_final_batch);
    assert_eq!(
        serde_json::to_value(&original.state).unwrap(),
        serde_json::to_value(&replay.state).unwrap(),
        "the serialized final diagnostics agree across same-seed raw replay"
    );
    assert!(
        !original_final_batch
            .resolution_prompt_state
            .as_ref()
            .expect("completed replay batch has final state")
            .active
    );
}

#[test]
fn prompt_state_survives_a_no_offer_concession_and_reconnect() {
    let decks = Some(vec![
        {
            let mut deck = vec!["brainstorm".to_string()];
            deck.extend(std::iter::repeat_n("island".to_string(), 29));
            deck
        },
        vec!["forest".into(); 30],
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        42_003,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "brainstorm");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let brainstorm_index = hand_index_for_card(&engine, 0, "brainstorm");
    engine
        .apply_scenario_command(0, &cast_spell(brainstorm_index, vec![]))
        .expect("cast Brainstorm");
    engine
        .apply_scenario_command(0, &pass())
        .expect("caster passes");
    engine
        .apply_scenario_command(1, &pass())
        .expect("second player passes");
    let prompt_batch = engine
        .apply_scenario_command(2, &pass())
        .expect("third player passes and resolves Brainstorm");
    let active_id = find_resolution_choice(&prompt_batch)
        .expect("Brainstorm offer")
        .resolution_prompt_id;
    assert_ne!(active_id, 0);

    let concession = engine
        .apply_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::Concede(Default::default())),
            },
        )
        .expect("unrelated player may concede while the offer remains active");
    assert!(find_resolution_choice(&concession).is_none());
    let state = concession
        .resolution_prompt_state
        .as_ref()
        .expect("concession batch carries final prompt state");
    assert!(state.active);
    assert_eq!(state.deciding_player_id, 0);
    assert_eq!(state.resolution_prompt_id, Some(active_id));

    let reconnect = engine.initial_response_batch();
    assert!(find_resolution_choice(&reconnect).is_none());
    let state = reconnect
        .resolution_prompt_state
        .as_ref()
        .expect("reconnect response carries prompt snapshot");
    assert!(state.active);
    assert_eq!(state.deciding_player_id, 0);
    assert_eq!(state.resolution_prompt_id, Some(active_id));
}

#[test]
fn brainstorm_face_is_an_ordinary_authored_program() {
    let face = tricerules_cards::registry::global()
        .get("brainstorm")
        .expect("Brainstorm registered")
        .primary_face();
    let normalized = serde_json::to_value(face).expect("face serializes for authoring consumers");

    assert!(
        normalized
            .get("spell_program")
            .is_some_and(|program| !program.is_null()),
        "Brainstorm needs the normalized spell_program consumed by the card review and authoring tools"
    );
    assert!(face.spell_effect.is_empty());
    assert!(face.custom_effect.is_none());
}

#[test]
fn twincast_copy_and_original_brainstorm_own_independent_private_programs() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        9_382,
        &[0, 1],
        20,
        Some(vec![vec!["island".into(); 30], vec!["forest".into(); 30]]),
        true,
    )
    .expect("copy fixture game");
    advance_to_main1_from_game_start(&mut engine);
    let original = inject_card_into_hand(&mut engine, 0, "brainstorm");
    let twincast = inject_card_into_hand(&mut engine, 1, "twincast");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "brainstorm");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast original Brainstorm");
    let original_generation = engine.state.zone_change_generation[&original];
    engine
        .apply_scenario_command(0, &pass())
        .expect("give P1 priority");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                slot,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: original,
                    expected_zone_change_generation: Some(original_generation),
                    ..Default::default()
                }],
            ),
        )
        .expect("Twincast targets the original spell");
    pass_priority_round(&mut engine);
    let copy = engine.state.stack.last().expect("copy on stack").clone();
    assert!(copy.is_copy);
    assert_eq!(copy.controller, 1);
    assert_ne!(copy.id, original);
    assert!(
        !engine.state.objects.contains_key(&copy.id),
        "the copy has no physical backing card"
    );
    assert_eq!(
        engine.state.objects[&twincast].zone,
        tricerules_core::Zone::Graveyard
    );
    let original_hand = engine.state.players[0].hand.clone();
    let original_library = engine.state.players[0].library.clone();

    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copy's private choice");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.source_object_id, copy.id);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!(
        pending.presentation.candidates,
        engine.state.players[1].hand
    );
    let ResolutionContinuation::HandChoice { stack, hand_choice } = &pending.continuation else {
        panic!("copy must use the ordinary private hand-choice transaction");
    };
    assert!(stack.item.is_copy && hand_choice.program_selection);
    assert!(!serde_json::to_value(stack).unwrap()["program"].is_null());
    let copy_selected: Vec<_> = pending
        .presentation
        .candidates
        .iter()
        .take(2)
        .copied()
        .collect();
    let before_wrong_actor = serde_json::to_value(&engine.state).unwrap();
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(copy_selected.clone()))
        .is_err());
    assert_eq!(
        serde_json::to_value(&engine.state).unwrap(),
        before_wrong_actor
    );
    engine
        .apply_scenario_command(1, &submit_resolution_choice(copy_selected.clone()))
        .expect("finish copy");
    assert_eq!(engine.state.players[1].library[0], copy_selected[1]);
    assert_eq!(engine.state.players[1].library[1], copy_selected[0]);
    assert_eq!(engine.state.players[0].hand, original_hand);
    assert_eq!(engine.state.players[0].library, original_library);
    assert!(!engine.state.objects.contains_key(&copy.id));
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, original);
    assert_eq!(
        engine.state.objects[&original].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        engine.state.zone_change_generation[&original],
        original_generation
    );
    assert_eq!(count_card_id_in_graveyard(&engine, 1, "brainstorm"), 0);
    let copy_hand_after = engine.state.players[1].hand.clone();
    let copy_library_after = engine.state.players[1].library.clone();

    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("original's fresh private choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.source_object_id, original);
    assert_eq!(
        pending.presentation.candidates,
        engine.state.players[0].hand
    );
    assert!(pending
        .presentation
        .candidates
        .iter()
        .all(|oid| !copy_selected.contains(oid)));
    let ResolutionContinuation::HandChoice { stack, hand_choice } = &pending.continuation else {
        panic!("original must use the ordinary private hand-choice transaction");
    };
    assert!(!stack.item.is_copy && hand_choice.program_selection);
    let original_program = serde_json::to_value(stack).unwrap();
    assert!(
        original_program["program"]["slots"]
            .as_array()
            .unwrap()
            .iter()
            .all(serde_json::Value::is_null),
        "the original starts with fresh unfilled results after the copy has finished"
    );
    let original_selected: Vec<_> = pending
        .presentation
        .candidates
        .iter()
        .take(2)
        .copied()
        .collect();
    engine
        .apply_scenario_command(0, &submit_resolution_choice(original_selected.clone()))
        .expect("finish original");
    assert_eq!(engine.state.players[0].library[0], original_selected[1]);
    assert_eq!(engine.state.players[0].library[1], original_selected[0]);
    assert_eq!(engine.state.players[1].hand, copy_hand_after);
    assert_eq!(engine.state.players[1].library, copy_library_after);
    assert_eq!(
        engine.state.objects[&original].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&original],
        original_generation + 1
    );
    assert_eq!(count_card_id_in_graveyard(&engine, 0, "brainstorm"), 1);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 3);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 3);
    assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
}

#[test]
fn nested_program_keeps_entered_if_and_parent_tail_across_private_child_choices() {
    // Both entered conditions see six cards. The nested true branch discards one, changing that
    // count to five while the same leaf parks for its private choice and Library of Leng's
    // replacement choice. Both levels explicitly yield a typed discard receipt; the captured
    // nested branch, its child draw, and the parent tail must each finish exactly once.
    let nested_program = r#"(
        id: "nested_program",
        name: "Nested Program",
        face_id: "nested_program",
        mana_cost: "{U}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "outer_branch", bind: ["outer_receipt"], kind: If(
                    condition: CardsInHand(
                        players: Relative(Controller), min: Some(6), max: Some(6)),
                    yes: (steps: [
                        (id: "inner_branch", bind: ["inner_receipt"], kind: If(
                            condition: CardsInHand(
                                players: Relative(Controller), min: Some(6), max: Some(6)),
                            yes: (steps: [
                                (id: "inner_discard_yes", bind: ["inner_yes_receipt"], kind: Perform(
                                    Discard(who: Controller, quantity: Exact(1))))
                            ], yields: ["inner_yes_receipt"]),
                            no: (steps: [
                                (id: "inner_discard_no", bind: ["inner_no_receipt"], kind: Perform(
                                    Discard(who: Controller, quantity: Exact(2))))
                            ], yields: ["inner_no_receipt"])
                        )),
                        (id: "child_draw", kind: Perform(Draw(count: 1)))
                    ], yields: ["inner_receipt"]),
                    no: (steps: [
                        (id: "outer_discard_no", bind: ["outer_no_receipt"], kind: Perform(
                            Discard(who: Controller, quantity: Exact(2))))
                    ], yields: ["outer_no_receipt"])
                )),
                (id: "parent_tail_draw", kind: Perform(Draw(count: 1)))
            ],
            yields: []
        )))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                nested_program,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/library_of_leng.ron"),
            ],
            &[],
        )
        .expect("nested program fixture registry validates"),
    ));
    let decks = Some(vec![
        vec![
            "nested_program".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec!["forest".into(); 30],
    ]);
    let mut engine =
        GameEngine::new(registry, 9_381, &[0, 1], 20, decks, true).expect("nested program engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    inject_library_card(&mut engine, 0, "island");
    inject_library_card(&mut engine, 0, "island");

    let first = cast_instant_and_resolve(
        &mut engine,
        0,
        "nested_program",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let discard_choice = find_resolution_choice(&first).expect("child discard choice");
    assert_eq!(discard_choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((discard_choice.min, discard_choice.max), (1, 1));
    assert_eq!(
        engine.state.players[0].hand.len(),
        6,
        "both nested If conditions enter while the hand count is six"
    );
    let selected = *engine.state.players[0]
        .hand
        .first()
        .expect("one card is available to discard");

    let replacement = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("answer private child hand choice");
    let replacement_choice = find_resolution_choice(&replacement)
        .expect("Library of Leng replacement choice remains inside the child instruction");
    assert_eq!(
        replacement_choice.choice_kind(),
        ChoiceKind::PrivateReplacement
    );
    assert_eq!((replacement_choice.min, replacement_choice.max), (1, 1));

    let completed = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![1]))
        .expect("put the discarded card on top of the library");
    assert!(find_resolution_choice(&completed).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        2,
        "the captured child draw and parent tail each complete exactly once"
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        7,
        "one discard changes six cards to five, then child and parent each draw once"
    );
}

#[test]
fn authored_permanent_choice_refreshes_after_candidate_owner_concedes() {
    let program = r#"(
        id: "departed_choice_program",
        name: "Departed Choice Program",
        face_id: "departed_choice_program",
        mana_cost: "{U}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "draw", bind: ["draw_receipt"], kind: Perform(
                    Draw(count: 1, who: Controller))),
                (id: "choose_land", bind: ["chosen_land"], kind: Perform(
                    ChoosePermanents(
                        chooser: Controller,
                        filter: (kind: AnyPermanent, permanent_types: [Land]),
                        min: 1,
                        max: 1
                    ))),
                (id: "untap_land", input: Some("chosen_land"), kind: Perform(
                    UntapChosenPermanents))
            ],
            yields: []
        )))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("departed-choice fixture registry validates"),
    ));
    let decks = Some(vec![
        vec!["island".into(); 30],
        vec!["forest".into(); 30],
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(registry, 9_383, &[0, 1, 2], 20, decks, true)
        .expect("departed-choice fixture engine");
    advance_to_main1_from_game_start(&mut engine);

    let departed_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let surviving_land = inject_permanent_on_battlefield(&mut engine, 2, "forest");
    let unselected_land = inject_permanent_on_battlefield(&mut engine, 2, "island");
    engine.state.objects.get_mut(&departed_land).unwrap().tapped = true;
    engine
        .state
        .objects
        .get_mut(&surviving_land)
        .unwrap()
        .tapped = true;
    engine
        .state
        .objects
        .get_mut(&unselected_land)
        .unwrap()
        .tapped = true;
    let spell = inject_card_into_hand(&mut engine, 0, "departed_choice_program");
    let hand_before_cast = engine.state.players[0].hand.len();
    let draws_before_cast = engine.state.turn_history.current.player(0).cards_drawn;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "departed_choice_program");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast targetless choice program");
    pass_priority_round(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("land choice after exactly one draw");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::PermanentObjects
    );
    assert_eq!(
        pending.presentation.candidates,
        vec![departed_land, surviving_land, unselected_land]
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before_cast + 1
    );
    let stack_generation = engine.state.zone_change_generation[&spell];

    engine
        .apply_scenario_command(1, &concede())
        .expect("P1 concedes");
    assert!(!engine.state.objects.contains_key(&departed_land));
    let refreshed = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P0's land choice refreshes after P1 leaves");
    assert_eq!(refreshed.deciding_player, 0);
    assert_eq!(
        refreshed.presentation.candidates,
        vec![surviving_land, unselected_land]
    );

    let before_stale_answer = engine.diagnostic_snapshot().expect("diagnostic snapshot");
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![departed_land]))
        .is_err());
    assert_eq!(
        engine.diagnostic_snapshot().expect("diagnostic snapshot"),
        before_stale_answer,
        "a removed candidate cannot partially mutate the refreshed offer"
    );

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![surviving_land]))
        .expect("choose P2's surviving land");
    assert!(!engine.state.objects[&surviving_land].tapped);
    assert!(engine.state.objects[&unselected_land].tapped);
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before_cast + 1,
        "refreshing and finishing the choice do not repeat the draw"
    );
    assert_eq!(
        engine.state.objects[&spell].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&spell],
        stack_generation + 1,
        "the physical spell exits the stack once"
    );
    assert_eq!(
        count_card_id_in_graveyard(&engine, 0, "departed_choice_program"),
        1
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.players[0].has_lost && !engine.state.players[2].has_lost);
}

fn cast_cost_choose_registry(
    include_twincast: bool,
) -> &'static tricerules_card_model::CardRegistry {
    let receipt_program = r#"(
        id: "choose_cast_receipt_fixture",
        name: "Choose Cast Receipt Fixture",
        face_id: "choose_cast_receipt_fixture",
        mana_cost: "{0}",
        types: ["Instant"],
        cast_cost_groups: [(
            group_id: "kicker_group", presentation: Fallback, min: 0, max: 1,
            options: [Mana(option_id: "kicker_option", presentation: Fallback,
                kind: Kicker, cost: "{1}")]
        )],
        spell_program: Some((body: (steps: [
            (id: "cast_receipt_choice", kind: Choose(
                choice: "cast_receipt_choice", chooser: Controller, optional: false,
                branches: [
                    (branch_id: "kicker_selected", presentation: Fallback, cost: None,
                        requirement: CastCostReceipt((group_id: "kicker_group",
                            option_id: "kicker_option", expected_selected: true)),
                        body: (steps: [(id: "draw_one", kind: Perform(Draw(
                            who: Controller, count: 1)))], yields: [])),
                    (branch_id: "kicker_not_selected", presentation: Fallback, cost: None,
                        requirement: CastCostReceipt((group_id: "kicker_group",
                            option_id: "kicker_option", expected_selected: false)),
                        body: (steps: [(id: "draw_two", kind: Perform(Draw(
                            who: Controller, count: 2)))], yields: [])),
                ],
                otherwise: (steps: [], yields: []),
            )),
        ], yields: []), target_bindings: []))
    )"#;
    let mut definitions = vec![
        receipt_program,
        include_str!("../../../tricerules-cards/data/island.ron"),
        include_str!("../../../tricerules-cards/data/forest.ron"),
    ];
    if include_twincast {
        definitions.push(include_str!("../../../tricerules-cards/data/twincast.ron"));
    }
    Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(&definitions, &[])
            .expect("cast-receipt program binds its exact owner group and option"),
    ))
}

fn cast_cost_mode_choose_registry() -> &'static tricerules_card_model::CardRegistry {
    let definition = r#"(
        id: "choose_mode_cast_receipt_fixture",
        name: "Choose Mode Cast Receipt Fixture",
        face_id: "choose_mode_cast_receipt_fixture",
        mana_cost: "{0}",
        types: ["Instant"],
        cast_cost_groups: [(
            group_id: "kicker_group", presentation: Fallback, min: 0, max: 1,
            options: [Mana(option_id: "kicker_option", presentation: Fallback,
                kind: Kicker, cost: "{1}")]
        )],
        modal_spell: (
            min_modes: 1, max_modes: 1,
            modes: [(mode_id: "receipt_mode", presentation: Fallback, effects: [],
                program: Some((body: (steps: [
                    (id: "mode_receipt_choice", kind: Choose(
                        choice: "mode_receipt_choice", chooser: Controller, optional: false,
                        branches: [
                            (branch_id: "kicker_selected", presentation: Fallback, cost: None,
                                requirement: CastCostReceipt((group_id: "kicker_group",
                                    option_id: "kicker_option", expected_selected: true)),
                                body: (steps: [(id: "draw_one", kind: Perform(Draw(
                                    who: Controller, count: 1)))], yields: [])),
                            (branch_id: "kicker_not_selected", presentation: Fallback, cost: None,
                                requirement: CastCostReceipt((group_id: "kicker_group",
                                    option_id: "kicker_option", expected_selected: false)),
                                body: (steps: [(id: "draw_two", kind: Perform(Draw(
                                    who: Controller, count: 2)))], yields: [])),
                        ],
                        otherwise: (steps: [], yields: []),
                    ))
                ], yields: []), target_bindings: []))) ]
        )
    )"#;
    let definitions = [
        definition,
        include_str!("../../../tricerules-cards/data/island.ron"),
        include_str!("../../../tricerules-cards/data/forest.ron"),
    ];
    Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(&definitions, &[])
            .expect("selected modal mode binds the exact face-owned cast-cost group"),
    ))
}

#[test]
fn program_choose_modal_mode_reads_its_exact_cast_cost_receipt() {
    use tricerules_proto::ruled::v1::{ruled_command::Cmd, CastCostGroupSelection};

    let mut engine = GameEngine::new(
        cast_cost_mode_choose_registry(),
        9_388,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("modal cast-receipt program engine");
    advance_to_main1_from_game_start(&mut engine);

    inject_card_into_hand(&mut engine, 0, "choose_mode_cast_receipt_fixture");
    let unselected_slot = hand_index_for_card(&engine, 0, "choose_mode_cast_receipt_fixture");
    let before_unselected = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &cast_modal_spell(unselected_slot, vec![(0, vec![])]))
        .expect("cast the selected face mode without the optional kicker");
    let unselected_resolution = pass_both_players_with_batch(&mut engine);
    assert!(
        find_resolution_choice(&unselected_resolution).is_none(),
        "the mandatory Choose auto-enters its sole free unselected-receipt branch"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        before_unselected + 2,
        "the exact unselected receipt still routes to Draw two"
    );
    assert!(engine.state.stack.is_empty());

    inject_card_into_hand(&mut engine, 0, "choose_mode_cast_receipt_fixture");
    let selected_slot = hand_index_for_card(&engine, 0, "choose_mode_cast_receipt_fixture");
    engine.state.players[0].mana_pool.colorless = 1;
    let before_selected = engine.state.turn_history.current.player(0).cards_drawn;
    let mut selected_cast = cast_modal_spell(selected_slot, vec![(0, vec![])]);
    let Some(Cmd::CastSpell(cast)) = selected_cast.cmd.as_mut() else {
        panic!("modal cast helper returns CastSpell")
    };
    cast.cast_cost_group_selections = vec![CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        ..Default::default()
    }];
    engine
        .apply_scenario_command(0, &selected_cast)
        .expect("cast the selected face mode with its exact kicker option");
    assert_eq!(
        engine.state.stack.last().unwrap().cast_cost_receipts[0]
            .group_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("kicker_group")
    );
    assert_eq!(
        engine.state.stack.last().unwrap().cast_cost_receipts[0]
            .option_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("kicker_option")
    );
    let selected_resolution = pass_both_players_with_batch(&mut engine);
    assert_eq!(
        find_resolution_choice(&selected_resolution),
        None,
        "the mandatory Choose auto-enters its sole free selected-receipt branch"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        before_selected + 1,
        "the exact selected receipt still routes to Draw one"
    );
    assert!(engine.state.stack.is_empty());
}

fn draw_limit_choose_registry() -> &'static tricerules_card_model::CardRegistry {
    let free_draw = r#"(
        id: "choose_free_draw_fixture", name: "Choose Free Draw Fixture",
        face_id: "choose_free_draw_fixture", mana_cost: "{0}", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "free_draw_choice", kind: Choose(
                choice: "free_draw_choice", chooser: Controller, optional: false,
                branches: [(branch_id: "draw", presentation: Fallback, cost: None,
                    body: (steps: [(id: "draw_one", kind: Perform(Draw(
                        who: Controller, count: 1)))], yields: []))],
                otherwise: (steps: [], yields: []),
            )),
        ], yields: []), target_bindings: []))
    )"#;
    let paid_draw = r#"(
        id: "choose_paid_draw_fixture", name: "Choose Paid Draw Fixture",
        face_id: "choose_paid_draw_fixture", mana_cost: "{0}", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "paid_draw_choice", kind: Choose(
                choice: "paid_draw_choice", chooser: Controller, optional: false,
                branches: [(branch_id: "pay_draw", presentation: Fallback,
                    cost: Mana("{1}"), body: (steps: [
                        (id: "draw_one", kind: Perform(Draw(
                            who: Controller, count: 1)))
                    ], yields: []))],
                otherwise: (steps: [], yields: []),
            )),
        ], yields: []), target_bindings: []))
    )"#;
    let definitions = [
        free_draw,
        paid_draw,
        include_str!("../../../tricerules-cards/data/narset,_parter_of_veils.ron"),
        include_str!("../../../tricerules-cards/data/island.ron"),
        include_str!("../../../tricerules-cards/data/forest.ron"),
    ];
    Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(&definitions, &[])
            .expect("draw Choose fixtures and Narset use the supported registry paths"),
    ))
}

#[test]
fn program_choose_suppresses_impossible_free_draw_but_keeps_paid_draw_offer() {
    let mut engine = GameEngine::new(
        draw_limit_choose_registry(),
        9_387,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("draw-limit Choose fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 1, "narset,_parter_of_veils");
    move_ready_to_battlefield(&mut engine, 1, "narset,_parter_of_veils");
    engine.state.turn_history.current.player_mut(0).cards_drawn = 1;

    inject_card_into_hand(&mut engine, 0, "choose_free_draw_fixture");
    let free_slot = hand_index_for_card(&engine, 0, "choose_free_draw_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(free_slot, vec![]))
        .expect("cast free draw fixture");
    let free_resolution = pass_both_players_with_batch(&mut engine);
    assert!(
        find_resolution_choice(&free_resolution).is_none(),
        "Narset makes the sole free Draw branch unavailable, so Choose takes Otherwise without prompting"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        1,
        "the suppressed free branch does not attempt a draw"
    );

    inject_card_into_hand(&mut engine, 0, "choose_paid_draw_fixture");
    let paid_slot = hand_index_for_card(&engine, 0, "choose_paid_draw_fixture");
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_scenario_command(0, &cast_spell(paid_slot, vec![]))
        .expect("cast paid draw fixture");
    let paid_resolution = pass_both_players_with_batch(&mut engine);
    let menu = find_resolution_choice(&paid_resolution)
        .expect("the fixed-cost Draw branch remains available under Narset");
    assert_eq!(
        menu.resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![(0, "Pay {1}", true)]
    );
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: menu.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("select the paid draw branch");
    assert_eq!(
        find_resolution_choice(
            &submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana,)
                .expect("pay the exact fixed cost")
        )
        .map(|choice| choice.choice_kind),
        None,
        "paying resolves the draw leaf directly rather than reopening Choose"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 1);
}

#[test]
fn program_choose_cast_cost_receipt_routes_selected_and_unselected_casts() {
    use tricerules_proto::ruled::v1::CastCostGroupSelection;

    let mut engine = GameEngine::new(
        cast_cost_choose_registry(false),
        9_386,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("cast-receipt program engine");
    advance_to_main1_from_game_start(&mut engine);

    inject_card_into_hand(&mut engine, 0, "choose_cast_receipt_fixture");
    let unselected_slot = hand_index_for_card(&engine, 0, "choose_cast_receipt_fixture");
    let draws_before_unselected = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(0, &cast_spell(unselected_slot, vec![]))
        .expect("cast without the optional kicker");
    assert!(engine
        .state
        .stack
        .last()
        .unwrap()
        .cast_cost_receipts
        .is_empty());
    let unselected_resolved = pass_both_players_with_batch(&mut engine);
    assert!(
        find_resolution_choice(&unselected_resolved).is_none(),
        "the mandatory Choose auto-enters its sole free unselected-receipt branch"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before_unselected + 2,
        "the captured unselected kicker receipt routes to Draw two"
    );
    assert!(engine.state.stack.is_empty());

    inject_card_into_hand(&mut engine, 0, "choose_cast_receipt_fixture");
    let selected_slot = hand_index_for_card(&engine, 0, "choose_cast_receipt_fixture");
    engine.state.players[0].mana_pool.colorless = 1;
    let draws_before_selected = engine.state.turn_history.current.player(0).cards_drawn;
    engine
        .apply_scenario_command(
            0,
            &cast_spell_with_cast_cost_groups(
                selected_slot,
                vec![],
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    ..Default::default()
                }],
            ),
        )
        .expect("cast with the exact selected kicker option");
    assert_eq!(
        engine.state.stack.last().unwrap().cast_cost_receipts[0]
            .group_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("kicker_group")
    );
    assert_eq!(
        engine.state.stack.last().unwrap().cast_cost_receipts[0]
            .option_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("kicker_option")
    );
    let selected_resolved = pass_both_players_with_batch(&mut engine);
    assert!(
        find_resolution_choice(&selected_resolved).is_none(),
        "the mandatory Choose auto-enters its sole free selected-receipt branch"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before_selected + 1,
        "the captured selected kicker receipt routes to Draw one"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn program_choose_cast_cost_receipt_survives_spell_copy() {
    use tricerules_proto::ruled::v1::CastCostGroupSelection;

    let mut engine = GameEngine::new(
        cast_cost_choose_registry(true),
        9_387,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["twincast"]),
            deck_with("island", &["twincast"]),
        ]),
        true,
    )
    .expect("copy cast-receipt program engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "choose_cast_receipt_fixture");
    let program_slot = hand_index_for_card(&engine, 0, "choose_cast_receipt_fixture");
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_scenario_command(
            0,
            &cast_spell_with_cast_cost_groups(
                program_slot,
                vec![],
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    ..Default::default()
                }],
            ),
        )
        .expect("P0 casts with the kicker receipt");
    let original = engine
        .state
        .stack
        .last()
        .expect("original on stack")
        .clone();
    assert_eq!(original.cast_cost_receipts.len(), 1);

    engine.state.players[1].mana_pool.blue = 2;
    inject_card_into_hand(&mut engine, 1, "twincast");
    let twincast_slot = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_scenario_command(0, &pass())
        .expect("P0 passes to Twincast");
    engine
        .apply_scenario_command(1, &cast_spell(twincast_slot, target_object(original.id)))
        .expect("P1 copies the kicked spell");
    pass_both_players_with_batch(&mut engine);
    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy)
        .expect("Twincast puts a copy on the stack");
    assert_eq!(copy.cast_cost_receipts, original.cast_cost_receipts);
    let p1_draws_before_copy = engine.state.turn_history.current.player(1).cards_drawn;
    let p0_draws_before_copy = engine.state.turn_history.current.player(0).cards_drawn;
    let copy_resolution = pass_both_players_with_batch(&mut engine);
    assert!(
        find_resolution_choice(&copy_resolution).is_none(),
        "the copy's mandatory Choose auto-enters its sole free receipt branch"
    );
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        p1_draws_before_copy + 1,
        "the copied spell's exact kicker receipt draws one card for P1"
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        p0_draws_before_copy,
        "the copy does not draw for the original spell's controller"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the original remains on the stack"
    );
    assert_eq!(engine.state.stack[0].id, original.id);

    let p0_draws_before_original = engine.state.turn_history.current.player(0).cards_drawn;
    let p1_draws_before_original = engine.state.turn_history.current.player(1).cards_drawn;
    let original_resolution = pass_both_players_with_batch(&mut engine);
    assert!(
        find_resolution_choice(&original_resolution).is_none(),
        "the original's mandatory Choose auto-enters its sole free receipt branch"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        p0_draws_before_original + 1,
        "the original spell's exact kicker receipt draws one card for P0"
    );
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        p1_draws_before_original,
        "the original spell does not draw for the copy's controller"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn program_damage_replacement_preserves_cursor_and_runs_following_tail_once() {
    use tricerules_core::state::{
        ActiveDamagePrevention, DamagePreventionAmount, DamagePreventionScope,
        ReplacementSourcePresentation,
    };

    let program = r#"(
        id: "program_damage_replacement_fixture",
        name: "Program Damage Replacement Fixture",
        face_id: "program_damage_replacement_fixture",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "before", bind: ["before"], kind: Perform(Draw(
                    who: Controller, count: 1
                ))),
                (id: "damage", kind: Perform(DamageTarget(
                    amount: 1, target: (kind: OpponentPlayer)
                ))),
                (id: "after", bind: ["after"], kind: Perform(Draw(
                    who: Controller, count: 1
                )))
            ], yields: []
        ), target_bindings: [
            (instruction_id: "damage", role: Target, target_group_index: 0)
        ])),
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose an opposing player", effect_indices: [])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("DamageTarget program fixture compiles"),
    ));
    let decks = Some(vec![
        vec!["program_damage_replacement_fixture".into(); 30],
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(registry, 9_481, &[0, 1], 20, decks, true)
        .expect("damage replacement fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let first_effect_id = engine.state.next_damage_prevention_effect_id;
    engine.state.damage_prevention_effects = [first_effect_id, first_effect_id + 1]
        .into_iter()
        .map(|id| ActiveDamagePrevention {
            id,
            static_origin: None,
            source_id: None,
            source_label: format!("test prevention {id}"),
            source_presentation: ReplacementSourcePresentation::default(),
            scope: DamagePreventionScope::Recipient(1),
            amount: DamagePreventionAmount::All,
            duration: tricerules_card_model::primitives::EffectDuration::UntilEndOfTurn,
            additional_effect: None,
        })
        .collect();
    engine.state.next_damage_prevention_effect_id = first_effect_id + 2;
    inject_card_into_hand(&mut engine, 0, "program_damage_replacement_fixture");
    let spell_slot = hand_index_for_card(&engine, 0, "program_damage_replacement_fixture");
    let before_draws = engine.state.turn_history.current.player(0).cards_drawn;
    let before_life = engine.state.players[1].life;
    engine
        .apply_scenario_command(0, &cast_spell(spell_slot, target_player(1)))
        .expect("cast the targeted program");
    let batch = pass_both_players_with_batch(&mut engine);
    let replacement =
        find_resolution_choice(&batch).expect("two applicable prevention effects require a choice");
    assert_eq!(
        replacement.choice_kind(),
        tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect
    );
    assert_eq!(replacement.deciding_player_id, 1);
    assert_eq!(replacement.replacement_options.len(), 2);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("damage replacement owns the suspended program");
    assert!(matches!(
        &pending.continuation,
        tricerules_core::state::ResolutionContinuation::DamageReplacement { .. }
    ));

    let application_id = replacement.replacement_options[0].application_id;
    let replacement_batch = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![application_id]))
        .expect("resolve one damage replacement and resume the program");
    assert!(find_resolution_choice(&replacement_batch).is_none());
    assert_eq!(engine.state.players[1].life, before_life);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        before_draws + 2,
        "the prefix and following tail each execute once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn program_choose_departed_controller_selects_delegate_then_runs_survivor_tail_once() {
    use tricerules_proto::ruled::v1::{
        ruled_command::Cmd, ResolutionChoiceDecision, SubmitResolutionChoice,
    };

    let program = r#"(
        id: "departed_program_choose",
        name: "Departed Program Choose",
        face_id: "departed_program_choose",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "choose_route", kind: Choose(
                    choice: "choose_route", chooser: Controller, optional: false,
                    branches: [
                        (branch_id: "route_a", presentation: Fallback, cost: None,
                            body: (steps: [], yields: [])),
                        (branch_id: "route_b", presentation: OracleLines([2]), cost: None,
                            body: (steps: [], yields: [])),
                    ],
                    otherwise: (steps: [], yields: []),
                )),
                (id: "damage_survivor", kind: Perform(DamageTarget(
                    amount: 3, target: (kind: OpponentPlayer))))
            ], yields: []
        ), target_bindings: [
            (instruction_id: "damage_survivor", role: Target, target_group_index: 0)
        ])),
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose a surviving opponent", effect_indices: [])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("the synthetic all-free Choose program validates"),
    ));
    let decks = Some((0..4).map(|_| vec!["forest".into(); 30]).collect());
    let mut engine = GameEngine::new(registry, 9_384, &[0, 1, 2, 3], 20, decks, true)
        .expect("four-player program Choose engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "departed_program_choose");
    let slot = hand_index_for_card(&engine, 0, "departed_program_choose");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_player(3)))
        .expect("P0 casts the targeted synthetic program");
    let mut initial_menu_batch = None;
    for _ in 0..4 {
        let actor = engine.state.priority_player_id();
        initial_menu_batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("every live player passes before the spell resolves"),
        );
    }
    let initial_menu_batch = initial_menu_batch.expect("the final pass produces the menu");
    let initial_menu = find_resolution_choice(&initial_menu_batch)
        .expect("the live controller receives the original all-free menu");
    assert_eq!(initial_menu.deciding_player_id, 0);
    assert_eq!(
        initial_menu
            .resolution_branches
            .iter()
            .map(|branch| branch.branch_index)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );

    let departed = engine
        .apply_scenario_command(0, &concede())
        .expect("P0 can concede while their spell is parked at Choose");
    assert!(engine.state.players[0].has_lost);
    let selector = find_resolution_choice(&departed)
        .expect("the next live player must choose a delegate for P0's choice");
    assert_eq!(selector.deciding_player_id, 1);
    assert_ne!(selector.resolution_prompt_id, 0);
    assert_eq!(
        selector
            .resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "P1 makes the choice", true),
            (1, "P2 makes the choice", true),
            (2, "P3 makes the choice", true),
        ]
    );

    let mut choose_delegate = SubmitResolutionChoice {
        resolution_prompt_id: selector.resolution_prompt_id,
        decision: ResolutionChoiceDecision::SelectBranch as i32,
        selected_branch_index: 1,
        ..Default::default()
    };
    let delegated = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(choose_delegate.clone())),
            },
        )
        .expect("P1 assigns P2 to make the original choice");
    let original_menu = find_resolution_choice(&delegated)
        .expect("P2 receives P0's original branch menu exactly once");
    assert_eq!(original_menu.deciding_player_id, 2);
    let delegated_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original Program choice remains parked for its live delegate");
    assert_eq!(delegated_pending.deciding_player, 2);
    let tricerules_core::state::ResolutionContinuation::ProgramChoose { stack, .. } =
        &delegated_pending.continuation
    else {
        panic!("the delegated prompt retains the original Program continuation");
    };
    assert_eq!(
        stack.item.controller, 0,
        "the effect controller remains P0 while P2 is the live decision maker"
    );
    assert_ne!(original_menu.resolution_prompt_id, 0);
    assert_eq!(
        original_menu
            .resolution_branches
            .iter()
            .map(|branch| (branch.branch_index, branch.selectable))
            .collect::<Vec<_>>(),
        vec![(0, true), (1, true)]
    );
    let oracle_presented = original_menu
        .resolution_branches
        .iter()
        .find(|branch| branch.branch_index == 1)
        .expect("the delegated original menu retains the second branch at its authored index");
    assert_eq!(oracle_presented.label, "Continue");
    let presentation = oracle_presented
        .presentation
        .as_ref()
        .expect("delegated original menu carries its owner-bound presentation path");
    assert_eq!(presentation.card_id, "departed_program_choose");
    assert_eq!(presentation.face_id, "departed_program_choose");
    assert_eq!(presentation.oracle_line_indices, vec![2]);
    assert_eq!(
        presentation.path.last().map(|part| part.id.as_str()),
        Some("route_b")
    );
    assert_eq!(presentation.fallback_text, "Continue");

    let stale_selector_state = engine
        .diagnostic_snapshot()
        .expect("snapshot before stale answer");
    let stale_selector_seed = engine.state.seed;
    let stale_selector_command_index = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(choose_delegate.clone())),
            },
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after stale answer"),
        stale_selector_state
    );
    assert_eq!(engine.state.seed, stale_selector_seed);
    assert_eq!(engine.state.command_index, stale_selector_command_index);

    let delegate_departure = engine
        .apply_scenario_command(2, &concede())
        .expect("P2 departs before making the delegated choice");
    assert!(engine.state.players[2].has_lost);
    let refreshed_selector = find_resolution_choice(&delegate_departure)
        .expect("P1 must choose another live delegate after P2 leaves");
    assert_eq!(refreshed_selector.deciding_player_id, 1);
    assert!(refreshed_selector.resolution_prompt_id > selector.resolution_prompt_id);
    assert_eq!(
        refreshed_selector
            .resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "P1 makes the choice", true),
            (1, "P3 makes the choice", true),
        ]
    );

    let self_delegate = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: refreshed_selector.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("P1 may take over the choice as a live delegate");
    let original_menu = find_resolution_choice(&self_delegate)
        .expect("the self-delegate receives the original menu once");
    assert_eq!(original_menu.deciding_player_id, 1);
    assert!(original_menu.resolution_prompt_id > refreshed_selector.resolution_prompt_id);

    choose_delegate.resolution_prompt_id = original_menu.resolution_prompt_id;
    choose_delegate.selected_branch_index = 1;
    let before_life = engine.state.players[3].life;
    let completed = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(choose_delegate)),
            },
        )
        .expect("P1 makes one selection after becoming the delegate");
    assert!(find_resolution_choice(&completed).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[3].life, before_life - 3);
    assert!(engine.state.players[1].life == 20 && engine.state.players[2].life == 20);
}

#[test]
fn program_choose_departed_selector_keeps_delegate_indices_stable() {
    use tricerules_proto::ruled::v1::{
        ruled_command::Cmd, ResolutionChoiceDecision, SubmitResolutionChoice,
    };

    let program = r#"(
        id: "departed_selector_program_choose",
        name: "Departed Selector Program Choose",
        face_id: "departed_selector_program_choose",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (
            steps: [
                (id: "choose_route", kind: Choose(
                    choice: "choose_route", chooser: Controller, optional: false,
                    branches: [
                        (branch_id: "route_a", presentation: Fallback, cost: None,
                            body: (steps: [], yields: [])),
                        (branch_id: "route_b", presentation: Fallback, cost: None,
                            body: (steps: [], yields: [])),
                    ],
                    otherwise: (steps: [], yields: []),
                )),
                (id: "damage_survivor", kind: Perform(DamageTarget(
                    amount: 3, target: (kind: OpponentPlayer))))
            ], yields: []
        ), target_bindings: [
            (instruction_id: "damage_survivor", role: Target, target_group_index: 0)
        ])),
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose a surviving opponent", effect_indices: [])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("the synthetic all-free Choose program validates"),
    ));
    let decks = Some((0..4).map(|_| vec!["forest".into(); 30]).collect());
    let mut engine = GameEngine::new(registry, 9_385, &[0, 1, 2, 3], 20, decks, true)
        .expect("four-player program Choose engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "departed_selector_program_choose");
    let slot = hand_index_for_card(&engine, 0, "departed_selector_program_choose");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_player(3)))
        .expect("P0 casts the targeted synthetic program");
    let mut initial_menu_batch = None;
    for _ in 0..4 {
        let actor = engine.state.priority_player_id();
        initial_menu_batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("every live player passes before the spell resolves"),
        );
    }
    assert!(find_resolution_choice(&initial_menu_batch.unwrap()).is_some());

    let first_departure = engine
        .apply_scenario_command(0, &concede())
        .expect("P0 leaves during the original menu");
    let selector = find_resolution_choice(&first_departure).expect("P1 selects a delegate");
    assert_eq!(selector.deciding_player_id, 1);
    assert_eq!(selector.resolution_branches.len(), 3);
    assert!(selector
        .resolution_branches
        .iter()
        .all(|branch| branch.selectable));

    let second_departure = engine
        .apply_scenario_command(1, &concede())
        .expect("P1 leaves while the delegate selector offer is pending");
    assert!(engine.state.players[1].has_lost);
    let refreshed =
        find_resolution_choice(&second_departure).expect("P2 takes over the delegate selection");
    assert_eq!(refreshed.deciding_player_id, 2);
    assert_ne!(refreshed.resolution_prompt_id, 0);
    assert_eq!(
        refreshed
            .resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "Unavailable player", false),
            (1, "P2 makes the choice", true),
            (2, "P3 makes the choice", true),
        ],
        "departed candidates stay as unselectable tombstones at their original indices"
    );

    let stale = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: selector.resolution_prompt_id,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 2,
            ..Default::default()
        })),
    };
    let stale_state = engine
        .diagnostic_snapshot()
        .expect("before stale selector answer");
    assert!(engine.apply_scenario_command(1, &stale).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("after stale selector answer"),
        stale_state
    );

    let delegated = engine
        .apply_scenario_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: refreshed.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 2,
                    ..Default::default()
                })),
            },
        )
        .expect("P2 chooses P3 using the preserved candidate index two");
    let original_menu = find_resolution_choice(&delegated).expect("P3 receives the original menu");
    assert_eq!(original_menu.deciding_player_id, 3);
    assert_eq!(original_menu.resolution_branches.len(), 2);

    let before_life = engine.state.players[3].life;
    engine
        .apply_scenario_command(
            3,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: original_menu.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("P3 selects one original branch");
    assert_eq!(engine.state.players[3].life, before_life - 3);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

/// Putting cards back on top of the library must emit **no** `PermanentMoved`. That event is
/// `FIELD_VISIBILITY_PUBLIC` down to its `card_id`, so announcing a hand → library move would
/// tell the opponent exactly which two cards Brainstorm hid on top — hand and library are hidden
/// zones (CR 400.2) and reach each player only through the redacted per-player zone view.
/// The silence is the feature; this test is what stops it being "fixed" into a leak.
#[test]
fn brainstorm_put_back_emits_no_public_move_event() {
    let decks = Some(vec![
        {
            let mut d = vec!["brainstorm".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        42,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "brainstorm");

    cast_instant_and_resolve(
        &mut e,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let chosen: Vec<u32> = e.state.players[0].hand.iter().take(2).copied().collect();
    let card_ids: Vec<String> = chosen
        .iter()
        .map(|oid| e.state.objects[oid].card_id.clone())
        .collect();
    let batch = e
        .apply_scenario_command(0, &submit_resolution_choice(chosen.clone()))
        .expect("submit brainstorm choice");

    for moved in permanents_moved_in(&batch) {
        assert!(
            !chosen.contains(&moved.object_id),
            "put-back object {} was announced publicly (destination {:?})",
            moved.object_id,
            moved.destination()
        );
        assert!(
            !card_ids.contains(&moved.card_id),
            "put-back card '{}' was announced publicly (destination {:?})",
            moved.card_id,
            moved.destination()
        );
    }
}

fn make_brainstorm_commander_engine(seed: u64, players: &[i32], commanders: &[&str]) -> GameEngine {
    let decks = Some(
        players
            .iter()
            .enumerate()
            .map(|(index, _)| {
                if index == 0 {
                    tricerules_core::EngineDeck {
                        mainboard: std::iter::once("brainstorm".to_string())
                            .chain(std::iter::repeat_n("island".to_string(), 29))
                            .collect(),
                        commanders: commanders.iter().map(|id| (*id).to_string()).collect(),
                    }
                } else {
                    tricerules_core::EngineDeck {
                        mainboard: vec!["forest".into(); 30],
                        commanders: Vec::new(),
                    }
                }
            })
            .collect(),
    );
    GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        decks,
        true,
    )
    .expect("Brainstorm Commander fixture engine")
}

fn put_declared_commander_in_hand(
    engine: &mut GameEngine,
    player_index: usize,
    commander_index: usize,
) -> (u32, u64) {
    let commander =
        engine.state.players[player_index].declared_commander_object_ids[commander_index];
    let player_id = engine.state.players[player_index].id;
    engine.state.players[player_index]
        .command_zone
        .retain(|object_id| *object_id != commander);
    engine.state.players[player_index].hand.push(commander);
    let object = engine
        .state
        .objects
        .get_mut(&commander)
        .expect("commander object");
    assert_eq!(object.owner, player_id);
    object.zone = tricerules_core::Zone::Hand;
    let generation = engine
        .state
        .zone_change_generation
        .get(&commander)
        .copied()
        .unwrap_or(0)
        + 1;
    engine
        .state
        .zone_change_generation
        .insert(commander, generation);
    (commander, generation)
}

fn cast_brainstorm_and_resolve_for_all_players(engine: &mut GameEngine) -> RuledEventBatch {
    cast_brainstorm_and_resolve_with_controller(engine, 0, 0)
}

fn cast_brainstorm_and_resolve_with_controller(
    engine: &mut GameEngine,
    caster_index: usize,
    controller_index: usize,
) -> RuledEventBatch {
    ensure_in_hand(engine, caster_index, "brainstorm");
    let caster_player_id = engine.state.players[caster_index].id;
    give_mana(
        engine,
        caster_player_id,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let spell_slot = hand_index_for_card(engine, caster_index, "brainstorm");
    engine
        .apply_scenario_command(caster_player_id, &cast_spell(spell_slot, vec![]))
        .expect("cast Brainstorm");
    if caster_index != controller_index {
        engine
            .state
            .stack
            .last_mut()
            .expect("Brainstorm is on the stack")
            .controller = engine.state.players[controller_index].id;
    }
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass before Brainstorm resolves");
        if engine.state.pending_resolution.is_some() {
            return batch;
        }
    }
    panic!("Brainstorm did not reach its hand-selection prompt");
}

fn submit_brainstorm_destination(branch_index: u32, prompt_id: u64) -> RuledCommand {
    let mut command = submit_resolution_decision(
        tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch,
    );
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
        command.cmd.as_mut()
    else {
        unreachable!("resolution decision helper creates a choice command")
    };
    answer.resolution_prompt_id = prompt_id;
    answer.selected_branch_index = branch_index;
    command
}

fn assert_brainstorm_private_destination_prompt(
    batch: &RuledEventBatch,
    card_name: &str,
) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
    assert_brainstorm_private_destination_prompt_for_player(batch, card_name, 0)
}

fn assert_brainstorm_private_destination_prompt_for_player(
    batch: &RuledEventBatch,
    card_name: &str,
    deciding_player_id: i32,
) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    let choice = find_resolution_choice(batch).expect("commander destination prompt");
    assert_eq!(choice.deciding_player_id, deciding_player_id);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.candidate_object_ids.is_empty());
    assert!(choice.candidate_card_ids.is_empty());
    assert!(choice.candidate_names.is_empty());
    assert!(choice.public_reveal.is_none());
    assert!(!choice.prompt_text.contains(card_name));
    assert_eq!(
        choice
            .resolution_branches
            .iter()
            .map(|branch| branch.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Command zone", "Library"]
    );

    let named_logs = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::Log(log)) if log.text.contains(card_name) => Some(log),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !named_logs.is_empty(),
        "the owner needs the selected card's name"
    );
    assert!(named_logs.iter().all(|log| {
        log.visible_to_player_id == Some(deciding_player_id) && log.hidden_from_player_id.is_none()
    }));
    for log in batch.events.iter().filter_map(|event| match &event.ev {
        Some(Ev::Log(log)) => Some(log),
        _ => None,
    }) {
        if log.visible_to_player_id != Some(deciding_player_id) {
            assert!(
                !log.text.contains(card_name),
                "a public or opponent-visible log disclosed {card_name}"
            );
        }
    }
    choice
}

fn assert_rejected_brainstorm_choice_is_immutable(
    engine: &mut GameEngine,
    actor: i32,
    command: &RuledCommand,
    label: &str,
) {
    let state_before = format!("{:?}", engine.state);
    let seed_before = engine.state.seed;
    let command_index_before = engine.state.command_index;
    assert!(
        engine.apply_scenario_command(actor, command).is_err(),
        "{label} must be rejected"
    );
    assert_eq!(format!("{:?}", engine.state), state_before);
    assert_eq!(engine.state.seed, seed_before);
    assert_eq!(engine.state.command_index, command_index_before);
}

#[test]
fn brainstorm_selected_commander_waits_for_private_library_replacement_choice() {
    use tricerules_core::Zone;

    let mut engine =
        make_brainstorm_commander_engine(42, &[0, 1], &["captain_storm,_cosmium_raider"]);
    advance_to_main1_from_game_start(&mut engine);

    // Model an earlier effect having put the exact declared commander into its owner's hand.
    // Keep the generation receipt current so Brainstorm's choice binds this physical card.
    let (commander, commander_generation) = put_declared_commander_in_hand(&mut engine, 0, 0);
    let draw = cast_brainstorm_and_resolve_for_all_players(&mut engine);
    let hand_choice = find_resolution_choice(&draw).expect("Brainstorm hand selection");
    assert_eq!(hand_choice.choice_kind(), ChoiceKind::HandCards);
    assert!(hand_choice.candidate_object_ids.contains(&commander));
    let another = hand_choice
        .candidate_object_ids
        .iter()
        .copied()
        .find(|object_id| *object_id != commander)
        .expect("one other hand card");
    let library_before = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();

    let replacement_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander, another]))
        .expect("choose the commander and another card for Brainstorm's ordered return");
    let replacement = assert_brainstorm_private_destination_prompt(
        &replacement_batch,
        "Captain Storm, Cosmium Raider",
    );
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before,
        "the whole selected cohort stays put until the replacement decision is answered"
    );
    assert!(engine.state.players[0].hand.contains(&commander));
    assert_eq!(engine.state.objects[&commander].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation.get(&commander),
        Some(&commander_generation)
    );

    // Choosing Library commits both selected cards in the established last-selected-on-top order.
    let library_choice = engine
        .apply_scenario_command(
            0,
            &submit_brainstorm_destination(1, replacement.resolution_prompt_id),
        )
        .expect("commander may use the Library replacement");
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.players[0].hand.contains(&commander));
    assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
    let library_after = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(library_after.len(), library_before.len() + 2);
    assert_eq!(&library_after[..2], &[another, commander]);
    assert_eq!(
        engine.state.zone_change_generation.get(&commander),
        Some(&(commander_generation + 1))
    );
    assert!(!permanents_moved_in(&library_choice)
        .iter()
        .any(|moved| moved.object_id == commander));
}

#[test]
fn brainstorm_collects_two_commander_replacements_before_any_cohort_move() {
    use tricerules_core::Zone;
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    // The fixture records the two declared identities but does not validate partner legality.
    let mut engine = make_brainstorm_commander_engine(
        43,
        &[0, 1],
        &[
            "captain_storm,_cosmium_raider",
            "vial_smasher,_gleeful_grenadier",
        ],
    );
    advance_to_main1_from_game_start(&mut engine);
    let (first, first_generation) = put_declared_commander_in_hand(&mut engine, 0, 0);
    let (second, second_generation) = put_declared_commander_in_hand(&mut engine, 0, 1);
    let hand_prompt = cast_brainstorm_and_resolve_for_all_players(&mut engine);
    let hand_choice = find_resolution_choice(&hand_prompt).expect("Brainstorm hand choice");
    assert!(hand_choice.candidate_object_ids.contains(&first));
    assert!(hand_choice.candidate_object_ids.contains(&second));
    let library_before = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();

    let first_offer = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![first, second]))
        .expect("select both declared commanders");
    let first_choice =
        assert_brainstorm_private_destination_prompt(&first_offer, "Captain Storm, Cosmium Raider");
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before
    );

    // The first card chooses Command, but remains in hand until the second replacement is known.
    let second_offer = engine
        .apply_scenario_command(
            0,
            &submit_brainstorm_destination(0, first_choice.resolution_prompt_id),
        )
        .expect("choose Command for the first commander");
    let second_choice = assert_brainstorm_private_destination_prompt(
        &second_offer,
        "Vial Smasher, Gleeful Grenadier",
    );
    assert_ne!(
        first_choice.resolution_prompt_id, second_choice.resolution_prompt_id,
        "each sequential commander gets a distinct prompt generation"
    );
    assert!(permanents_moved_in(&second_offer).is_empty());
    assert!(engine.state.players[0].hand.contains(&first));
    assert!(engine.state.players[0].hand.contains(&second));
    assert_eq!(engine.state.objects[&first].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&second].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation.get(&first),
        Some(&first_generation)
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&second),
        Some(&second_generation)
    );
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before
    );

    let mut stale_prompt = submit_brainstorm_destination(1, second_choice.resolution_prompt_id);
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
        stale_prompt.cmd.as_mut()
    else {
        unreachable!()
    };
    answer.resolution_prompt_id = first_choice.resolution_prompt_id;
    assert_rejected_brainstorm_choice_is_immutable(
        &mut engine,
        0,
        &stale_prompt,
        "stale prior commander prompt ID",
    );

    let committed = engine
        .apply_scenario_command(
            0,
            &submit_brainstorm_destination(1, second_choice.resolution_prompt_id),
        )
        .expect("choose Library for the second commander");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&first].zone, Zone::Command);
    assert_eq!(engine.state.objects[&second].zone, Zone::Library);
    assert_eq!(
        engine.state.zone_change_generation.get(&first),
        Some(&(first_generation + 1))
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&second),
        Some(&(second_generation + 1))
    );
    assert_eq!(engine.state.players[0].command_zone.last(), Some(&first));
    assert_eq!(engine.state.players[0].library.front(), Some(&second));
    let moved = permanents_moved_in(&committed);
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].object_id, first);
    assert_eq!(
        moved[0].destination(),
        tricerules_proto::ruled::v1::permanent_moved::Destination::Command
    );
    assert!(committed.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == first)
    )));
    assert!(committed.events.iter().all(|event| match &event.ev {
        Some(Ev::CardsRevealed(reveal)) => reveal.cards.iter().all(|card| card.object_id != second),
        Some(Ev::Log(log)) if log.visible_to_player_id != Some(0) => {
            !log.text.contains("Vial Smasher, Gleeful Grenadier")
        }
        _ => true,
    }));
}

#[test]
fn brainstorm_rejects_wrong_actor_decline_malformed_branch_and_stale_generation_atomically() {
    use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

    let mut engine =
        make_brainstorm_commander_engine(44, &[0, 1], &["captain_storm,_cosmium_raider"]);
    advance_to_main1_from_game_start(&mut engine);
    let (commander, _) = put_declared_commander_in_hand(&mut engine, 0, 0);
    let hand_prompt = cast_brainstorm_and_resolve_for_all_players(&mut engine);
    let hand_choice = find_resolution_choice(&hand_prompt).expect("Brainstorm hand choice");
    let another = hand_choice
        .candidate_object_ids
        .iter()
        .copied()
        .find(|object_id| *object_id != commander)
        .expect("one other hand card");
    let offer = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander, another]))
        .expect("select commander for replacement");
    let choice =
        assert_brainstorm_private_destination_prompt(&offer, "Captain Storm, Cosmium Raider");

    let valid = submit_brainstorm_destination(1, choice.resolution_prompt_id);
    assert_rejected_brainstorm_choice_is_immutable(&mut engine, 1, &valid, "wrong actor");

    let mut decline = submit_resolution_decision(ResolutionChoiceDecision::Decline);
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
        decline.cmd.as_mut()
    else {
        unreachable!()
    };
    answer.resolution_prompt_id = choice.resolution_prompt_id;
    assert_rejected_brainstorm_choice_is_immutable(&mut engine, 0, &decline, "decline");

    let invalid_branch = submit_brainstorm_destination(2, choice.resolution_prompt_id);
    assert_rejected_brainstorm_choice_is_immutable(
        &mut engine,
        0,
        &invalid_branch,
        "out-of-range branch",
    );

    let mut malformed = valid.clone();
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
        malformed.cmd.as_mut()
    else {
        unreachable!()
    };
    answer.chosen_object_ids.push(another);
    assert_rejected_brainstorm_choice_is_immutable(&mut engine, 0, &malformed, "malformed branch");

    let generation = engine
        .state
        .zone_change_generation
        .get(&commander)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(commander, generation + 1);
    assert_rejected_brainstorm_choice_is_immutable(
        &mut engine,
        0,
        &valid,
        "stale selected generation",
    );
}

#[test]
fn brainstorm_unrelated_departure_refreshes_private_offer_without_moving_the_cohort() {
    let mut engine =
        make_brainstorm_commander_engine(45, &[0, 1, 2], &["captain_storm,_cosmium_raider"]);
    advance_to_main1_from_game_start(&mut engine);
    let (commander, generation) = put_declared_commander_in_hand(&mut engine, 0, 0);
    let hand_prompt = cast_brainstorm_and_resolve_for_all_players(&mut engine);
    let hand_choice = find_resolution_choice(&hand_prompt).expect("Brainstorm hand choice");
    let another = hand_choice
        .candidate_object_ids
        .iter()
        .copied()
        .find(|object_id| *object_id != commander)
        .expect("one other hand card");
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander, another]))
        .expect("select commander for replacement");
    let library_before = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();

    let departure = engine
        .apply_scenario_command(2, &concede())
        .expect("unrelated player may leave during the private replacement offer");
    assert!(engine.state.players[2].has_lost);
    let refreshed =
        assert_brainstorm_private_destination_prompt(&departure, "Captain Storm, Cosmium Raider");
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.players[0].hand.contains(&commander));
    assert!(engine.state.players[0].hand.contains(&another));
    assert_eq!(
        engine.state.objects[&commander].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&commander),
        Some(&generation)
    );
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before
    );
    assert!(permanents_moved_in(&departure)
        .iter()
        .all(|moved| moved.object_id != commander && moved.object_id != another));

    let completed = engine
        .apply_scenario_command(
            0,
            &submit_brainstorm_destination(1, refreshed.resolution_prompt_id),
        )
        .expect("the refreshed live offer remains answerable");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&commander].zone,
        tricerules_core::Zone::Library
    );
    assert_eq!(
        engine.state.objects[&another].zone,
        tricerules_core::Zone::Library
    );
    let library_after = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(&library_after[..2], &[another, commander]);
    assert!(permanents_moved_in(&completed)
        .iter()
        .all(|moved| moved.object_id != commander && moved.object_id != another));
}

#[test]
fn brainstorm_caster_departure_discards_the_uncommitted_owned_frame() {
    let mut engine =
        make_brainstorm_commander_engine(46, &[0, 1, 2], &["captain_storm,_cosmium_raider"]);
    advance_to_main1_from_game_start(&mut engine);
    let (commander, _) = put_declared_commander_in_hand(&mut engine, 0, 0);
    let hand_prompt = cast_brainstorm_and_resolve_for_all_players(&mut engine);
    let hand_choice = find_resolution_choice(&hand_prompt).expect("Brainstorm hand choice");
    let another = hand_choice
        .candidate_object_ids
        .iter()
        .copied()
        .find(|object_id| *object_id != commander)
        .expect("one other hand card");
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander, another]))
        .expect("select commander for replacement");

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("caster may leave while their private replacement choice is pending");
    assert!(engine.state.players[0].has_lost);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        2,
        "the other players continue the game after the caster leaves"
    );
    assert!(engine.state.winner().is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(permanents_moved_in(&departure).iter().all(|moved| {
        (moved.object_id != commander && moved.object_id != another)
            || (moved.destination()
                != tricerules_proto::ruled::v1::permanent_moved::Destination::Command
                && moved.destination()
                    != tricerules_proto::ruled::v1::permanent_moved::Destination::Library)
    }));
}

#[test]
fn brainstorm_source_owner_departure_preserves_live_controller_cohort_and_commits_it() {
    use tricerules_core::{engine::EngineDeck, Zone};
    use tricerules_proto::ruled::v1::ruled_event::Ev;

    let decks = Some(vec![
        EngineDeck {
            mainboard: std::iter::once("brainstorm".to_string())
                .chain(std::iter::repeat_n("island".to_string(), 29))
                .collect(),
            commanders: Vec::new(),
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 30],
            commanders: vec!["captain_storm,_cosmium_raider".into()],
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 30],
            commanders: Vec::new(),
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        47,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("three-player Brainstorm source-owner fixture");
    advance_to_main1_from_game_start(&mut engine);
    let (commander, commander_generation) = put_declared_commander_in_hand(&mut engine, 1, 0);
    ensure_in_hand(&mut engine, 0, "brainstorm");
    let physical_spell =
        engine.state.players[0].hand[hand_index_for_card(&engine, 0, "brainstorm")];
    assert_eq!(engine.state.objects[&physical_spell].owner, 0);

    // Model a live P1 controller/cohort owner for P0's actual Brainstorm object. This is a
    // parked-state continuation fixture, not a claim about the legal cast/steal path.
    let hand_prompt = cast_brainstorm_and_resolve_with_controller(&mut engine, 0, 1);
    let hand_choice = find_resolution_choice(&hand_prompt).expect("P1's Brainstorm hand choice");
    assert_eq!(hand_choice.deciding_player_id, 1);
    let another = hand_choice
        .candidate_object_ids
        .iter()
        .copied()
        .find(|object_id| *object_id != commander)
        .expect("one other card from the live controller's hand");
    assert_eq!(engine.state.objects[&another].owner, 1);
    let selected = [commander, another];
    let generations = selected
        .iter()
        .map(|object_id| {
            engine
                .state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let library_before = engine.state.players[1]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let first_offer_batch = engine
        .apply_scenario_command(1, &submit_resolution_choice(selected.to_vec()))
        .expect("P1 selects the commander and another card for the ordered return");
    let first_offer = assert_brainstorm_private_destination_prompt_for_player(
        &first_offer_batch,
        "Captain Storm, Cosmium Raider",
        1,
    );
    let pending_before = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P1's private replacement choice is parked");
    let continuation_before = format!("{:?}", pending_before.continuation);
    let prompt_id_before = pending_before.resolution_prompt_id.get();
    assert_eq!(first_offer.resolution_prompt_id, prompt_id_before);

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("physical source owner P0 leaves while P1 controls the resolving Program");
    assert!(engine.state.players[0].has_lost);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        2
    );
    assert!(engine.state.winner().is_none());
    assert!(!engine.state.objects.contains_key(&physical_spell));
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectRemoved(removed)) if removed.object_id == physical_spell
            ))
            .count(),
        1,
        "the departed physical source exits exactly once"
    );
    let refreshed = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the live controller's pending hand return survives source-owner departure");
    assert_eq!(refreshed.deciding_player, 1);
    assert_eq!(refreshed.resolution_prompt_id.get(), prompt_id_before);
    assert_eq!(format!("{:?}", refreshed.continuation), continuation_before);
    assert_brainstorm_private_destination_prompt_for_player(
        &departure,
        "Captain Storm, Cosmium Raider",
        1,
    );
    assert!(selected.iter().all(|object_id| {
        engine.state.players[1].hand.contains(object_id)
            && engine.state.objects[object_id].zone == Zone::Hand
    }));
    assert_eq!(
        selected
            .iter()
            .map(|object_id| {
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>(),
        generations
    );
    assert_eq!(
        engine.state.players[1]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before
    );

    let completed = engine
        .apply_scenario_command(1, &submit_brainstorm_destination(1, prompt_id_before))
        .expect("P1 answers the preserved Library replacement");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(!engine.state.objects.contains_key(&physical_spell));
    assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
    assert_eq!(engine.state.objects[&another].zone, Zone::Library);
    assert_eq!(engine.state.players[1].library.front(), Some(&another));
    assert_eq!(engine.state.players[1].library.get(1), Some(&commander));
    assert_eq!(
        engine.state.zone_change_generation.get(&commander),
        Some(&(commander_generation + 1))
    );
    assert!(completed.events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| selected.contains(&card.object_id))
    )));
}

#[test]
fn brainstorm_rejects_card_not_in_hand_without_mutating() {
    let decks = Some(vec![
        {
            let mut d = vec!["brainstorm".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        42,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    cast_instant_and_resolve(
        &mut e,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert!(e.state.pending_resolution.is_some());

    let hand_snapshot = e.state.players[0].hand.clone();
    let lib_snapshot: Vec<u32> = e.state.players[0].library.iter().copied().collect();

    // One legal hand card + one library card (not a legal candidate) -> rejected, no mutation.
    let legal = e.state.players[0].hand[0];
    let library_card = e.state.players[0].library[0];
    let res = e.apply_scenario_command(0, &submit_resolution_choice(vec![legal, library_card]));
    assert!(res.is_err(), "returning a card not in hand is illegal");

    assert!(e.state.pending_resolution.is_some(), "still parked");
    assert_eq!(e.state.players[0].hand, hand_snapshot, "hand unchanged");
    assert_eq!(
        e.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<u32>>(),
        lib_snapshot,
        "library unchanged"
    );

    // Wrong count is also rejected.
    assert!(
        e.apply_scenario_command(0, &submit_resolution_choice(vec![legal]))
            .is_err(),
        "must choose exactly two"
    );
    // The opponent cannot answer the controller's choice.
    assert!(
        e.apply_scenario_command(
            1,
            &submit_resolution_choice(vec![legal, e.state.players[0].hand[1]])
        )
        .is_err(),
        "only the deciding player may submit"
    );
}

#[test]
fn brainstorm_resolution_is_deterministic() {
    let make = || {
        // Deck must stay large enough that Brainstorm's "draw three" never reaches an empty
        // library — otherwise the caster decks out (CR 104.3c) and the resolution is mooted.
        let decks = Some(vec![
            {
                let mut d = vec!["brainstorm".to_string()];
                d.extend(
                    ["island", "forest", "swamp", "mountain", "plains"]
                        .iter()
                        .cycle()
                        .take(29)
                        .map(|s| s.to_string()),
                );
                d
            },
            vec!["forest".into(); 30],
        ]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            1234,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
        advance_to_main1_from_game_start(&mut e);
        cast_instant_and_resolve(
            &mut e,
            0,
            "brainstorm",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        // Same choice command (indices into hand) on both runs.
        let chosen: Vec<u32> = e.state.players[0].hand.iter().take(2).copied().collect();
        e.apply_scenario_command(0, &submit_resolution_choice(chosen))
            .unwrap();
        e.state.players[0]
            .library
            .iter()
            .map(|&o| e.state.objects[&o].card_id.clone())
            .collect::<Vec<String>>()
    };
    assert_eq!(make(), make(), "same seed + same choices => same library");
}

#[test]
fn gifts_split_rejects_stale_selected_generation_without_mutating_the_prompt() {
    let (mut engine, found, split) = make_gifts_partition_prompt(8_702);
    let stale_selected = found[0];
    let generation = engine
        .state
        .zone_change_generation
        .get(&stale_selected)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(stale_selected, generation + 1);

    let mut answer = submit_resolution_choice(vec![found[0], found[1]]);
    let Some(Cmd::SubmitResolutionChoice(command)) = answer.cmd.as_mut() else {
        unreachable!()
    };
    command.resolution_prompt_id = split.resolution_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut engine,
        1,
        &answer,
        "split answer naming a cohort member with a stale generation",
    );
}

#[test]
fn gifts_split_rejects_stale_unselected_complement_without_mutating_the_prompt() {
    let (mut engine, found, split) = make_gifts_partition_prompt(8_703);
    let stale_complement = found[3];
    let generation = engine
        .state
        .zone_change_generation
        .get(&stale_complement)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(stale_complement, generation + 1);

    let mut answer = submit_resolution_choice(vec![found[0], found[1]]);
    let Some(Cmd::SubmitResolutionChoice(command)) = answer.cmd.as_mut() else {
        unreachable!()
    };
    command.resolution_prompt_id = split.resolution_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut engine,
        1,
        &answer,
        "split answer while its unselected complement has a stale generation",
    );
}

#[test]
fn gifts_ungiven_opponent_chooses_the_split() {
    use tricerules_core::Zone;
    // Minimal deck: inject specific distinct-name cards after setup so shuffle order
    // cannot affect which names are available in the library for the search.
    let decks = Some(vec![
        {
            let mut d = vec!["gifts_ungiven".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject four distinct-name cards so we always have exactly 4 to choose from.
    let lb_id = inject_library_card(&mut e, 0, "lightning_bolt");
    let div_id = inject_library_card(&mut e, 0, "divination");
    let ctsp_id = inject_library_card(&mut e, 0, "counterspell");
    let murd_id = inject_library_card(&mut e, 0, "murder");
    let found = vec![lb_id, div_id, ctsp_id, murd_id];
    e.state.players[0]
        .declared_commander_object_ids
        .push(ctsp_id);

    let batch = cast_targeted_instant_and_resolve(
        &mut e,
        0,
        "gifts_ungiven",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_player(1),
    );

    // First interrupt: the controller searches their library (up to four). This is a *private*
    // library search (ChoiceKind::LibrarySearch), so the relay redacts the candidate library
    // cards from the opponent — the library must not leak. Only the chosen cards become public.
    let search = find_resolution_choice(&batch).expect("search choice");
    assert!(search.public_reveal.is_none());
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!(
        search.choice_kind(),
        ChoiceKind::LibrarySearch,
        "private library search (not public-revealed)"
    );
    for &oid in &found {
        assert!(
            search.candidate_object_ids.contains(&oid),
            "injected card must be a search candidate"
        );
    }

    let mut wrong_search_actor = submit_resolution_choice(found.clone());
    let Some(Cmd::SubmitResolutionChoice(answer)) = wrong_search_actor.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = search.resolution_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        1,
        &wrong_search_actor,
        "wrong actor on the private search",
    );
    let mut stale_search_prompt = submit_resolution_choice(found.clone());
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_search_prompt.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = search.resolution_prompt_id + 1;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        0,
        &stale_search_prompt,
        "stale private-search prompt ID",
    );
    let initial_generation = e.state.zone_change_generation.get(&lb_id).copied();
    e.state
        .zone_change_generation
        .insert(lb_id, initial_generation.unwrap_or(0) + 1);
    let mut stale_candidate = submit_resolution_choice(found.clone());
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_candidate.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = search.resolution_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        0,
        &stale_candidate,
        "search candidate whose exact generation is stale",
    );
    if let Some(generation) = initial_generation {
        e.state.zone_change_generation.insert(lb_id, generation);
    } else {
        e.state.zone_change_generation.remove(&lb_id);
    }

    let batch2 = e
        .apply_scenario_command(0, &submit_resolution_choice(found.clone()))
        .expect("search submit");

    // Second interrupt: the OPPONENT chooses which two go to the controller's graveyard.
    let split = find_resolution_choice(&batch2).expect("opponent split choice");
    let reveal = split
        .public_reveal
        .as_ref()
        .expect("Gifts explicitly publishes its revealed set to everyone");
    assert_eq!(reveal.zone_owner_player_id, 0);
    assert_eq!(
        reveal
            .cards
            .iter()
            .map(|card| card.object_id)
            .collect::<Vec<_>>(),
        found
    );
    assert!(!reveal.reveal_id.is_empty());
    assert_eq!(
        batch2.events.iter().find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(receipt)) => Some(receipt),
            _ => None,
        }),
        Some(reveal),
        "the standalone reveal and split metadata share one immutable receipt"
    );
    assert_eq!(
        batch2
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_))))
            .count(),
        1,
        "one Gifts resolution emits one reveal event for its cohort"
    );
    let active_reveals = batch2
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ActivePublicRevealSnapshot(snapshot)) => Some(&snapshot.reveals),
            _ => None,
        })
        .expect("active public reveal snapshot");
    assert_eq!(
        active_reveals
            .iter()
            .find(|active| active.reveal_id == reveal.reveal_id),
        Some(reveal),
        "the active reveal projection keeps the exact cohort receipt while the split is pending"
    );
    assert_eq!(split.deciding_player_id, 1, "opponent decides the split");
    assert_eq!((split.min, split.max), (2, 2));
    assert!(
        !split.unique_names,
        "different names constrain the search, not the target's subset of the revealed cohort"
    );
    assert_eq!(
        split.candidate_object_ids, found,
        "split is over the revealed set"
    );

    let to_grave: Vec<u32> = found.iter().take(2).copied().collect();
    let to_hand: Vec<u32> = found.iter().skip(2).copied().collect();
    let split_prompt_id = split.resolution_prompt_id;
    let mut wrong_split_actor = submit_resolution_choice(to_grave.clone());
    let Some(Cmd::SubmitResolutionChoice(answer)) = wrong_split_actor.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = split_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        0,
        &wrong_split_actor,
        "wrong actor on the target's split",
    );
    let mut stale_split_prompt = submit_resolution_choice(to_grave.clone());
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_split_prompt.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = split_prompt_id + 1;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        1,
        &stale_split_prompt,
        "stale split prompt ID",
    );
    let mut over_cardinality = submit_resolution_choice(found[..3].to_vec());
    let Some(Cmd::SubmitResolutionChoice(answer)) = over_cardinality.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = split_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        1,
        &over_cardinality,
        "split selection over its maximum cardinality",
    );
    let mut duplicate_partition = submit_resolution_choice(vec![found[0], found[0]]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = duplicate_partition.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = split_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        1,
        &duplicate_partition,
        "duplicate physical object in split selection",
    );
    let commander_batch = e
        .apply_scenario_command(1, &submit_resolution_choice(to_grave.clone()))
        .expect("opponent submits split");
    let commander_choice = find_resolution_choice(&commander_batch)
        .expect("the hand-bound commander receives its CR 903.9b replacement choice");
    assert_eq!(commander_choice.deciding_player_id, 0);
    assert!(commander_choice.prompt_text.contains("903.9b"));
    assert_eq!(commander_choice.public_reveal.as_ref(), Some(reveal));
    assert!(found
        .iter()
        .all(|oid| e.state.objects[oid].zone == Zone::Library));
    let mut invalid_commander_branch = submit_resolution_choice(Vec::new());
    let Some(Cmd::SubmitResolutionChoice(answer)) = invalid_commander_branch.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = commander_choice.resolution_prompt_id;
    answer.decision = tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32;
    answer.selected_branch_index = 99;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        0,
        &invalid_commander_branch,
        "invalid CR 903.9b commander replacement branch",
    );
    assert!(found
        .iter()
        .all(|oid| e.state.objects[oid].zone == Zone::Library));
    let mut decline_command_zone = submit_resolution_choice(Vec::new());
    let Some(Cmd::SubmitResolutionChoice(answer)) = decline_command_zone.cmd.as_mut() else {
        unreachable!()
    };
    answer.decision = tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32;
    answer.selected_branch_index = 1;
    let order_batch = e
        .apply_scenario_command(0, &decline_command_zone)
        .expect("commander owner accepts the hand destination");
    let graveyard_order = find_resolution_choice(&order_batch)
        .expect("CR 404.3 asks for the graveyard order before any partition move");
    assert_eq!(graveyard_order.deciding_player_id, 0);
    assert_eq!(
        graveyard_order.choice_kind(),
        ChoiceKind::SimultaneousEntryOrder
    );
    assert_eq!(graveyard_order.candidate_object_ids, to_grave);
    assert_eq!(
        graveyard_order.public_reveal.as_ref(),
        Some(reveal),
        "the precommit order choice keeps the original public cohort receipt"
    );
    let mut malformed_graveyard_order = submit_resolution_choice(vec![to_grave[0], to_grave[0]]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed_graveyard_order.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = graveyard_order.resolution_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut e,
        0,
        &malformed_graveyard_order,
        "CR 404.3 order with a duplicate and missing partition member",
    );
    assert!(found
        .iter()
        .all(|oid| e.state.objects[oid].zone == Zone::Library));
    let active_reveals = order_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ActivePublicRevealSnapshot(snapshot)) => Some(&snapshot.reveals),
            _ => None,
        })
        .expect("active public reveal snapshot");
    assert_eq!(
        active_reveals
            .iter()
            .find(|active| active.reveal_id == reveal.reveal_id),
        Some(reveal),
        "both observers retain the exact unrevealed-movement cohort during CR 404.3 ordering"
    );
    for oid in &found {
        assert_eq!(
            e.state.objects[oid].zone,
            Zone::Library,
            "no partition member moves before every replacement and order choice is collected"
        );
    }

    let moved = e
        .apply_scenario_command(0, &submit_resolution_choice(to_grave.clone()))
        .expect("controller submits the graveyard order");

    assert!(e.state.pending_resolution.is_none(), "resolution completed");
    for oid in &to_grave {
        assert_eq!(
            e.state.objects[oid].zone,
            Zone::Graveyard,
            "chosen -> graveyard"
        );
        assert!(e.state.players[0].graveyard.contains(oid));
    }
    for oid in &to_hand {
        assert_eq!(e.state.objects[oid].zone, Zone::Hand, "rest -> hand");
        assert!(e.state.players[0].hand.contains(oid));
    }
    assert_eq!(e.state.objects[&ctsp_id].zone, Zone::Hand);
    assert_eq!(count_card_id_in_graveyard(&e, 0, "gifts_ungiven"), 1);
    assert_eq!(
        moved
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1,
        "Gifts shuffles its controller's library exactly once after moving the partition"
    );
}

#[test]
fn gifts_ungiven_search_and_partition_cover_zero_through_four_cards() {
    use tricerules_core::Zone;

    let names = ["lightning_bolt", "divination", "counterspell", "murder"];
    for count in 0..=4 {
        let decks = Some(vec![
            {
                let mut d = vec!["gifts_ungiven".to_string()];
                d.extend(std::iter::repeat_n("island".to_string(), 29));
                d
            },
            vec!["forest".into(); 30],
        ]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            700 + count as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
        advance_to_main1_from_game_start(&mut e);

        let found = names[..count]
            .iter()
            .map(|name| inject_library_card(&mut e, 0, name))
            .collect::<Vec<_>>();
        let search_batch = cast_targeted_instant_and_resolve(
            &mut e,
            0,
            "gifts_ungiven",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            target_player(1),
        );
        let search = find_resolution_choice(&search_batch).expect("search choice");
        assert_eq!(search.deciding_player_id, 0);
        assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
        assert!(search.public_reveal.is_none());
        assert!(found
            .iter()
            .all(|object_id| search.candidate_object_ids.contains(object_id)));

        let after_search = e
            .apply_scenario_command(0, &submit_resolution_choice(found.clone()))
            .expect("submit selected search cohort");
        let split = find_resolution_choice(&after_search);
        let mut after_partition = after_search.clone();
        if count > 0 {
            let chosen_count = count.min(2);
            let split = split.expect("opponent confirms the full smaller cohort or chooses two");
            assert_eq!(split.deciding_player_id, 1);
            assert_eq!(
                (split.min, split.max),
                (chosen_count as u32, chosen_count as u32)
            );
            assert_eq!(split.candidate_object_ids, found);
            assert!(split.public_reveal.is_some());
            assert!(!split.unique_names);
            after_partition = e
                .apply_scenario_command(
                    1,
                    &submit_resolution_choice(found.iter().take(chosen_count).copied().collect()),
                )
                .expect("opponent confirms/submits the partition");
        } else {
            assert!(split.is_none(), "an empty cohort needs no subset choice");
        }

        // With two or more cards, the selected graveyard cohort is ordered before any move
        // (CR 404.3). For >2 cards the opponent's earlier choice selected the first two.
        let grave_cards = found.iter().take(count.min(2)).copied().collect::<Vec<_>>();
        let mut moved = after_partition.clone();
        if grave_cards.len() >= 2 {
            let order_choice = find_resolution_choice(&after_partition)
                .expect("graveyard order is offered before the cohort moves");
            assert_eq!(
                order_choice.choice_kind(),
                ChoiceKind::SimultaneousEntryOrder
            );
            assert_eq!(order_choice.candidate_object_ids, grave_cards);
            let order = e
                .state
                .pending_resolution
                .as_ref()
                .expect("graveyard order pending")
                .presentation
                .candidates
                .clone();
            assert_eq!(order, grave_cards);
            moved = e
                .apply_scenario_command(
                    0,
                    &submit_resolution_choice(grave_cards.iter().rev().copied().collect()),
                )
                .expect("controller orders the graveyard cohort");
        } else {
            assert!(
                find_resolution_choice(&after_partition).is_none(),
                "the one-card confirmation is already complete and needs no ordering choice"
            );
        }

        assert!(e.state.pending_resolution.is_none(), "Gifts completes");
        for (index, object_id) in found.iter().enumerate() {
            let expected = if index < grave_cards.len() {
                Zone::Graveyard
            } else {
                Zone::Hand
            };
            assert_eq!(
                e.state.objects[object_id].zone, expected,
                "cohort size {count}"
            );
        }
        assert_eq!(
            moved
                .events
                .iter()
                .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
                .count(),
            1,
            "cohort size {count} shuffles exactly once"
        );
    }
}

#[test]
fn gifts_target_departing_during_search_chooses_a_live_partition_delegate() {
    use tricerules_core::Zone;

    let decks = Some(
        (0..4)
            .map(|player| {
                if player == 0 {
                    let mut deck = vec!["gifts_ungiven".to_string()];
                    deck.extend(std::iter::repeat_n("island".to_string(), 29));
                    deck
                } else {
                    vec!["forest".to_string(); 30]
                }
            })
            .collect(),
    );
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        704,
        &[0, 1, 2, 3],
        20,
        decks,
        true,
    )
    .expect("new four-player Gifts engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "gifts_ungiven");
    let found = [
        inject_library_card(&mut e, 0, "lightning_bolt"),
        inject_library_card(&mut e, 0, "divination"),
        inject_library_card(&mut e, 0, "counterspell"),
        inject_library_card(&mut e, 0, "murder"),
    ];
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let gifts = hand_index_for_card(&e, 0, "gifts_ungiven");
    e.apply_scenario_command(0, &cast_spell(gifts, target_player(1)))
        .expect("cast Gifts targeting P1");
    pass_priority_round(&mut e);
    let search_before_departure = e
        .state
        .pending_resolution
        .as_ref()
        .expect("controller searches before target leaves");
    assert_eq!(search_before_departure.deciding_player, 0);
    assert_eq!(
        search_before_departure.presentation.choice_kind,
        ChoiceKind::LibrarySearch
    );

    let departure = e
        .apply_scenario_command(1, &concede())
        .expect("target leaves while the search is pending");
    assert!(
        departure
            .events
            .iter()
            .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))),
        "leaving during a private search must not reveal or repeat a reveal"
    );
    assert!(
        e.state.players[1].has_lost,
        "the targeted player is marked departed"
    );
    let search = e
        .state
        .pending_resolution
        .as_ref()
        .expect("search offer remains for its live controller");
    assert_eq!(search.deciding_player, 0);
    assert!(found
        .iter()
        .all(|id| search.presentation.candidates.contains(id)));

    let after_search = e
        .apply_scenario_command(0, &submit_resolution_choice(found.to_vec()))
        .expect("controller submits the retained search cohort");
    let delegate_prompt = find_resolution_choice(&after_search)
        .expect("controller chooses a live player after the target departed during search");
    assert_eq!(delegate_prompt.deciding_player_id, 0);
    assert_eq!(delegate_prompt.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(delegate_prompt.candidate_player_ids, vec![2, 3]);

    let mut stale_target = submit_resolution_choice(vec![found[0], found[1]]);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = stale_target.cmd.as_mut() {
        answer.resolution_prompt_id = delegate_prompt.resolution_prompt_id;
    }
    assert!(
        e.apply_scenario_command(1, &stale_target).is_err(),
        "a departed target cannot answer the stale split command"
    );
    assert_eq!(
        e.state.pending_resolution.as_ref().unwrap().deciding_player,
        0,
        "rejecting the departed actor preserves the delegate prompt"
    );

    let mut choose_delegate = submit_resolution_choice(Vec::new());
    let Some(Cmd::SubmitResolutionChoice(answer)) = choose_delegate.cmd.as_mut() else {
        unreachable!()
    };
    answer.decision = tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32;
    answer.selected_branch_index = 0;
    let delegated = e
        .apply_scenario_command(0, &choose_delegate)
        .expect("controller delegates P1's choice to live opponent P2");
    let split = find_resolution_choice(&delegated).expect("P2 receives the split choice");
    let reveal = split
        .public_reveal
        .as_ref()
        .expect("retained public reveal");
    assert_eq!(split.deciding_player_id, 2);
    assert_eq!(split.candidate_object_ids, found);
    assert_eq!((split.min, split.max), (2, 2));
    assert!(!split.unique_names);
    assert_eq!(
        delegated
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_))))
            .count(),
        0,
        "delegation reuses the first reveal without emitting another"
    );
    let original_reveal = after_search
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(receipt)) => Some(receipt),
            _ => None,
        })
        .expect("the search result was revealed exactly once");
    assert_eq!(reveal, original_reveal);

    let order = e
        .apply_scenario_command(2, &submit_resolution_choice(vec![found[0], found[1]]))
        .expect("delegate chooses the two graveyard cards");
    let order_prompt = find_resolution_choice(&order).expect("controller orders the graveyard");
    assert_eq!(order_prompt.deciding_player_id, 0);
    assert_eq!(order_prompt.public_reveal.as_ref(), Some(original_reveal));
    let moved = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![found[1], found[0]]))
        .expect("controller submits the precommit graveyard order");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.objects[&found[0]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&found[1]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&found[2]].zone, Zone::Hand);
    assert_eq!(e.state.objects[&found[3]].zone, Zone::Hand);
    assert_eq!(count_card_id_in_graveyard(&e, 0, "gifts_ungiven"), 1);
    assert_eq!(
        moved
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
            ))
            .count(),
        1,
        "the delegated resolution shuffles exactly once"
    );
}

#[test]
fn gifts_target_departing_during_partition_chooses_a_live_delegate_without_revealing_again() {
    use tricerules_core::Zone;

    let decks = Some(
        (0..4)
            .map(|player| {
                if player == 0 {
                    let mut deck = vec!["gifts_ungiven".to_string()];
                    deck.extend(std::iter::repeat_n("island".to_string(), 29));
                    deck
                } else {
                    vec!["forest".to_string(); 30]
                }
            })
            .collect(),
    );
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        705,
        &[0, 1, 2, 3],
        20,
        decks,
        true,
    )
    .expect("new four-player Gifts engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "gifts_ungiven");
    let found = [
        inject_library_card(&mut e, 0, "lightning_bolt"),
        inject_library_card(&mut e, 0, "divination"),
        inject_library_card(&mut e, 0, "counterspell"),
        inject_library_card(&mut e, 0, "murder"),
    ];
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let gifts = hand_index_for_card(&e, 0, "gifts_ungiven");
    e.apply_scenario_command(0, &cast_spell(gifts, target_player(1)))
        .expect("cast Gifts targeting P1");
    pass_priority_round(&mut e);
    let search = e
        .state
        .pending_resolution
        .as_ref()
        .expect("controller searches");
    assert_eq!(search.presentation.choice_kind, ChoiceKind::LibrarySearch);
    let searched = e
        .apply_scenario_command(0, &submit_resolution_choice(found.to_vec()))
        .expect("controller completes the private search");
    let split = find_resolution_choice(&searched).expect("target P1 receives the partition");
    assert_eq!(split.deciding_player_id, 1);
    assert_eq!(split.candidate_object_ids, found);
    let receipt = split.public_reveal.clone().expect("one public reveal");
    assert_eq!(
        searched
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_))))
            .count(),
        1
    );

    let departure = e
        .apply_scenario_command(1, &concede())
        .expect("target leaves while the revealed partition is pending");
    assert!(e.state.players[1].has_lost);
    assert!(departure
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    let delegate_prompt =
        find_resolution_choice(&departure).expect("controller receives a delegate choice");
    assert_eq!(delegate_prompt.deciding_player_id, 0);
    assert_eq!(delegate_prompt.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(delegate_prompt.candidate_player_ids, vec![2, 3]);
    let published = departure.events.iter().find_map(|event| match &event.ev {
        Some(Ev::ActivePublicRevealSnapshot(snapshot)) => snapshot
            .reveals
            .iter()
            .find(|active| active.reveal_id == receipt.reveal_id),
        _ => None,
    });
    assert_eq!(published, Some(&receipt));

    let mut stale_target = submit_resolution_choice(found[..2].to_vec());
    if let Some(Cmd::SubmitResolutionChoice(answer)) = stale_target.cmd.as_mut() {
        answer.resolution_prompt_id = delegate_prompt.resolution_prompt_id;
    }
    assert!(e.apply_scenario_command(1, &stale_target).is_err());
    assert_eq!(
        e.state.pending_resolution.as_ref().unwrap().deciding_player,
        0
    );
    let mut choose_delegate = submit_resolution_choice(Vec::new());
    let Some(Cmd::SubmitResolutionChoice(answer)) = choose_delegate.cmd.as_mut() else {
        unreachable!()
    };
    answer.decision = tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32;
    answer.selected_branch_index = 0;
    let delegated = e
        .apply_scenario_command(0, &choose_delegate)
        .expect("P0 assigns P2 as the live chooser");
    let new_split = find_resolution_choice(&delegated).expect("P2 receives the split");
    assert_eq!(new_split.deciding_player_id, 2);
    assert_eq!(new_split.candidate_object_ids, found);
    assert_eq!(new_split.public_reveal.as_ref(), Some(&receipt));
    assert!(delegated
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    let order = e
        .apply_scenario_command(2, &submit_resolution_choice(found[..2].to_vec()))
        .expect("P2 chooses two graveyard cards");
    let order_prompt = find_resolution_choice(&order).expect("P0 orders the GY cohort");
    assert_eq!(order_prompt.deciding_player_id, 0);
    assert_eq!(order_prompt.public_reveal.as_ref(), Some(&receipt));
    let moved = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![found[1], found[0]]))
        .expect("P0 supplies the CR 404.3 order");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.objects[&found[0]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&found[1]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&found[2]].zone, Zone::Hand);
    assert_eq!(e.state.objects[&found[3]].zone, Zone::Hand);
    assert_eq!(
        moved
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
            ))
            .count(),
        1
    );
}

#[test]
fn gifts_source_owner_departure_preserves_surviving_controller_library_cohort() {
    use tricerules_core::Zone;

    let decks = Some(
        (0..4)
            .map(|player| {
                if player == 0 {
                    let mut deck = vec!["gifts_ungiven".to_string()];
                    deck.extend(std::iter::repeat_n("island".to_string(), 29));
                    deck
                } else {
                    vec!["forest".to_string(); 30]
                }
            })
            .collect(),
    );
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        706,
        &[0, 1, 2, 3],
        20,
        decks,
        true,
    )
    .expect("new four-player Gifts engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "gifts_ungiven");
    let found = [
        inject_library_card(&mut e, 1, "lightning_bolt"),
        inject_library_card(&mut e, 1, "divination"),
        inject_library_card(&mut e, 1, "counterspell"),
        inject_library_card(&mut e, 1, "murder"),
    ];
    let generations = found
        .iter()
        .map(|id| e.state.zone_change_generation.get(id).copied().unwrap_or(0))
        .collect::<Vec<_>>();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let gifts = hand_index_for_card(&e, 0, "gifts_ungiven");
    e.apply_scenario_command(0, &cast_spell(gifts, target_player(2)))
        .expect("P0 casts Gifts at P2");
    let physical_spell = e.state.stack.last().expect("Gifts on stack").id;
    assert_eq!(e.state.objects[&physical_spell].owner, 0);
    e.state.stack.last_mut().unwrap().controller = 1;
    pass_priority_round(&mut e);
    let search = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the resolving spell is controlled by P1");
    assert_eq!(search.deciding_player, 1);
    assert!(found
        .iter()
        .all(|id| search.presentation.candidates.contains(id)));

    let departure = e
        .apply_scenario_command(0, &concede())
        .expect("physical source owner P0 leaves while P1 controls the resolving program");
    assert!(e.state.players[0].has_lost);
    assert!(!e.state.objects.contains_key(&physical_spell));
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectRemoved(removed)) if removed.object_id == physical_spell
            ))
            .count(),
        1,
        "the departed physical spell exits the stack exactly once"
    );
    assert!(departure
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    let refreshed = e
        .state
        .pending_resolution
        .as_ref()
        .expect("P1's search offer survives source-owner departure");
    assert_eq!(refreshed.deciding_player, 1);
    assert!(found.iter().all(|id| {
        e.state.objects.get(id).is_some_and(|object| {
            object.owner == 1
                && object.zone == Zone::Library
                && e.state.players[1].library.contains(id)
        })
    }));

    let searched = e
        .apply_scenario_command(1, &submit_resolution_choice(found.to_vec()))
        .expect("surviving controller completes the original retained search");
    let split = find_resolution_choice(&searched).expect("target P2 chooses the partition");
    assert_eq!(split.deciding_player_id, 2);
    assert_eq!(split.candidate_object_ids, found);
    let receipt = split
        .public_reveal
        .clone()
        .expect("cohort was revealed once");
    assert_eq!(
        searched
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_))))
            .count(),
        1
    );
    let partition = e
        .apply_scenario_command(2, &submit_resolution_choice(found[..2].to_vec()))
        .expect("P2 chooses the graveyard half");
    let order = find_resolution_choice(&partition).expect("P1 orders their graveyard");
    assert_eq!(order.deciding_player_id, 1);
    assert_eq!(order.public_reveal.as_ref(), Some(&receipt));
    let moved = e
        .apply_scenario_command(1, &submit_resolution_choice(vec![found[1], found[0]]))
        .expect("P1 orders and completes the physical cohort");
    assert!(e.state.pending_resolution.is_none());
    assert!(!e.state.objects.contains_key(&physical_spell));
    assert_eq!(e.state.objects[&found[0]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&found[1]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&found[2]].zone, Zone::Hand);
    assert_eq!(e.state.objects[&found[3]].zone, Zone::Hand);
    for (id, before) in found.iter().zip(generations) {
        assert_eq!(e.state.zone_change_generation[id], before + 1);
    }
    assert_eq!(
        moved
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P1 shuffles their library."
            ))
            .count(),
        1
    );
    assert!(moved.events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::StackObjectRemoved(removed)) if removed.object_id == physical_spell
    )));
}

#[test]
fn gifts_source_owner_departure_during_precommit_keeps_controller_cohort_and_does_not_resurrect_spell(
) {
    use tricerules_core::Zone;

    let decks = Some(
        (0..4)
            .map(|player| {
                if player == 0 {
                    let mut deck = vec!["gifts_ungiven".to_string()];
                    deck.extend(std::iter::repeat_n("island".to_string(), 29));
                    deck
                } else {
                    vec!["forest".to_string(); 30]
                }
            })
            .collect(),
    );
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        707,
        &[0, 1, 2, 3],
        20,
        decks,
        true,
    )
    .expect("new four-player Gifts engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "gifts_ungiven");
    let found = [
        inject_library_card(&mut e, 1, "lightning_bolt"),
        inject_library_card(&mut e, 1, "divination"),
        inject_library_card(&mut e, 1, "counterspell"),
        inject_library_card(&mut e, 1, "murder"),
    ];
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let gifts = hand_index_for_card(&e, 0, "gifts_ungiven");
    e.apply_scenario_command(0, &cast_spell(gifts, target_player(2)))
        .expect("P0 casts Gifts at P2");
    let physical_spell = e.state.stack.last().expect("Gifts on stack").id;
    assert_eq!(e.state.objects[&physical_spell].owner, 0);
    e.state.stack.last_mut().unwrap().controller = 1;
    pass_priority_round(&mut e);
    let search = e
        .state
        .pending_resolution
        .as_ref()
        .expect("P1 controls the resolving search");
    assert_eq!(search.deciding_player, 1);
    let searched = e
        .apply_scenario_command(1, &submit_resolution_choice(found.to_vec()))
        .expect("P1 completes the library search");
    let split = find_resolution_choice(&searched).expect("P2 chooses the partition");
    assert_eq!(split.deciding_player_id, 2);
    let receipt = split.public_reveal.clone().expect("public cohort receipt");
    let order = e
        .apply_scenario_command(2, &submit_resolution_choice(found[..2].to_vec()))
        .expect("P2 chooses two cards for the graveyard");
    let order_prompt = find_resolution_choice(&order).expect("P1 receives the precommit order");
    assert_eq!(order_prompt.deciding_player_id, 1);
    assert_eq!(order_prompt.public_reveal.as_ref(), Some(&receipt));
    assert!(found
        .iter()
        .all(|id| e.state.objects[id].zone == Zone::Library));

    let departure = e
        .apply_scenario_command(0, &concede())
        .expect("source owner A leaves during the precommit movement prompt");
    assert!(e.state.players[0].has_lost);
    assert!(!e.state.objects.contains_key(&physical_spell));
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectRemoved(removed)) if removed.object_id == physical_spell
            ))
            .count(),
        1,
        "CR 800.4a publishes one removal for the parked physical spell"
    );
    assert!(departure
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    let refreshed = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the surviving controller retains the CR 404.3 prompt");
    assert_eq!(refreshed.deciding_player, 1);
    assert_eq!(refreshed.presentation.candidates, found[..2]);
    assert!(found.iter().all(|id| {
        e.state.objects.get(id).is_some_and(|object| {
            object.owner == 1
                && object.zone == Zone::Library
                && e.state.players[1].library.contains(id)
        })
    }));

    let moved = e
        .apply_scenario_command(1, &submit_resolution_choice(vec![found[1], found[0]]))
        .expect("P1 completes the still-live partition");
    assert!(e.state.pending_resolution.is_none());
    assert!(!e.state.objects.contains_key(&physical_spell));
    assert_eq!(e.state.objects[&found[0]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&found[1]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&found[2]].zone, Zone::Hand);
    assert_eq!(e.state.objects[&found[3]].zone, Zone::Hand);
    assert_eq!(
        moved
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P1 shuffles their library."
            ))
            .count(),
        1,
        "the surviving controller completes the explicit shuffle tail once"
    );
    assert!(moved.events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::StackObjectRemoved(removed)) if removed.object_id == physical_spell
    )));
    assert!(
        moved.events.iter().all(|event| !matches!(
            &event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == physical_spell
        )),
        "the resumed private frame does not publish a second stack exit"
    );
    assert!(
        !e.state.players[0].graveyard.contains(&physical_spell),
        "the missing physical spell is not resurrected into its former owner's graveyard"
    );
}

#[test]
fn gifts_controller_departure_during_precommit_exiles_survivor_owned_spell_once_and_prunes_cohort()
{
    use tricerules_core::Zone;

    let decks = Some(vec![
        deck_with("island", &["gifts_ungiven", "wan_shi_tong,_librarian"]),
        deck_with("forest", &[]),
        deck_with("swamp", &[]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        708,
        &[0, 1, 2, 3],
        20,
        decks,
        true,
    )
    .expect("new four-player Gifts engine");
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "gifts_ungiven");
    let wan = move_ready_to_battlefield(&mut e, 0, "wan_shi_tong,_librarian");
    pass_priority_round(&mut e);
    ensure_card_in_hand(&mut e, 0, "gifts_ungiven");
    let found = [
        inject_library_card(&mut e, 1, "lightning_bolt"),
        inject_library_card(&mut e, 1, "divination"),
        inject_library_card(&mut e, 1, "counterspell"),
        inject_library_card(&mut e, 1, "murder"),
    ];
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let gifts = hand_index_for_card(&e, 0, "gifts_ungiven");
    e.apply_scenario_command(0, &cast_spell(gifts, target_player(2)))
        .expect("P0 casts Gifts at P2");
    let physical_spell = e.state.stack.last().expect("Gifts on stack").id;
    assert_eq!(e.state.objects[&physical_spell].owner, 0);
    e.state.stack.last_mut().unwrap().controller = 1;
    pass_priority_round(&mut e);
    assert_eq!(
        e.state.pending_resolution.as_ref().unwrap().deciding_player,
        1
    );
    let searched = e
        .apply_scenario_command(1, &submit_resolution_choice(found.to_vec()))
        .expect("P1 searches their own library");
    let split = find_resolution_choice(&searched).expect("P2 chooses the split");
    assert_eq!(split.deciding_player_id, 2);
    let receipt = split.public_reveal.clone().expect("public reveal receipt");
    let order = e
        .apply_scenario_command(2, &submit_resolution_choice(found[..2].to_vec()))
        .expect("P2 selects the graveyard partition");
    let order_prompt = find_resolution_choice(&order).expect("P1 orders the graveyard side");
    assert_eq!(order_prompt.deciding_player_id, 1);
    assert_eq!(order_prompt.public_reveal.as_ref(), Some(&receipt));
    assert_eq!(
        e.state.staged_trigger_groups.len(),
        1,
        "Wan observes the search once"
    );
    let spell_generation = e
        .state
        .zone_change_generation
        .get(&physical_spell)
        .copied()
        .unwrap_or(0);
    assert!(found
        .iter()
        .all(|id| e.state.objects[id].zone == Zone::Library));

    let departure = e
        .apply_scenario_command(1, &concede())
        .expect("controller and library owner P1 leaves during precommit");
    assert!(e.state.players[1].has_lost);
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.zone_change_generation.get(&physical_spell).copied(),
        Some(spell_generation + 1),
        "the surviving owner's spell makes one true exit to exile"
    );
    assert_eq!(e.state.objects[&physical_spell].zone, Zone::Exile);
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::PermanentMoved(moved))
                    if moved.object_id == physical_spell
                        && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Exile as i32
            ))
            .count(),
        1,
        "the physical spell is visibly exiled once"
    );
    assert!(departure.events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::CardsRevealed(_) | Ev::ResolutionChoiceRequired(_))
    )));
    assert!(
        found.iter().all(|id| !e.state.objects.contains_key(id)),
        "departed B's library refs are pruned"
    );
    assert!(departure.events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P1 shuffles their library."
    )));
    assert_eq!(
        e.state
            .stack
            .iter()
            .filter(|item| item.source_permanent_id == Some(wan))
            .count(),
        1,
        "the already-staged LibrarySearched trigger remains single"
    );

    let hand_before_trigger = e.state.players[0].hand.len();
    pass_priority_round(&mut e);
    assert_eq!(
        e.state.objects[&wan].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(e.state.players[0].hand.len(), hand_before_trigger + 1);
    assert_eq!(e.state.objects[&physical_spell].zone, Zone::Exile);
    assert_eq!(
        e.state.zone_change_generation.get(&physical_spell).copied(),
        Some(spell_generation + 1),
        "finishing the frozen cursor neither resurrects nor moves the spell again"
    );
    assert_eq!(count_card_id_in_graveyard(&e, 0, "gifts_ungiven"), 0);
}

#[test]
fn intuition_comparator_reverses_the_typed_partition_without_unique_names() {
    use tricerules_core::Zone;

    // Test-only comparator. This deliberately adds no card data or admission: it proves the
    // shared typed cohort primitive also supports Intuition's opposite destination mapping.
    let intuition = r#"(
        id: "intuition_comparator_fixture", name: "Intuition Comparator Fixture",
        face_id: "intuition_comparator_fixture", mana_cost: "{3}{U}", types: ["Instant"],
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target opponent", effect_indices: [],
        )])),
        spell_program: Some((
            body: (steps: [
                (id: "search", bind: ["found"], input: None,
                    kind: Perform(SearchLibrary(
                        who: Controller, optional: false, count: 3,
                        count_by_cast_cost: None, filter: None,
                        selection_constraint: None, slots: [],
                        zones: Fixed([Library]), destination: KeepInLibrary,
                        conditional_destination: None, shuffle: false, reveal: false,
                        selection_bounds: Some((min: 3, max: 3)),
                    ))),
                (id: "reveal", bind: ["revealed"], input: Some("found"),
                    kind: Perform(RevealCohort)),
                (id: "choose_one", bind: ["partition"], input: Some("revealed"),
                    kind: Perform(ChooseCohortPartition(
                        chooser: TargetedPlayer(group_index: 0, kind: OpponentPlayer),
                        selection: ExactlyUpTo(1),
                    ))),
                (id: "move_partition", bind: [], input: Some("partition"),
                    kind: Perform(MoveCohortPartition(
                        selected_to: Hand, remainder_to: Graveyard,
                    ))),
                (id: "shuffle", bind: [], input: None,
                    kind: Perform(ShuffleLibrary(who: Controller))),
            ], yields: []),
            target_bindings: [
                (instruction_id: "choose_one", role: Target, target_group_index: 0),
            ],
        ))
    )"#;
    let duplicate_a = r#"(id: "intuition_duplicate_a", name: "Repeated Name",
        face_id: "intuition_duplicate_a", mana_cost: "{0}", types: ["Sorcery"])"#;
    let distinct = r#"(id: "intuition_distinct", name: "Distinct Name",
        face_id: "intuition_distinct", mana_cost: "{0}", types: ["Sorcery"])"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                intuition,
                duplicate_a,
                distinct,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("test-only Intuition comparator compiles through the same typed program"),
    ));

    let make_engine = |seed: u64, short_library: bool| {
        let p0_size = if short_library { 9 } else { 30 };
        let decks = Some(vec![
            vec!["island".to_string(); p0_size],
            vec!["forest".into(); 30],
        ]);
        let mut engine = GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
            .expect("new Intuition comparison engine");
        advance_to_main1_from_game_start(&mut engine);
        inject_card_into_hand(&mut engine, 0, "intuition_comparator_fixture");
        engine
    };

    // Three search results include two distinct physical cards with the same name. The search
    // admits both; the opponent puts one card in hand and the two-card graveyard side requires
    // the controller's CR 404.3 order. Run once for the command-zone choice and once for the
    // graveyard branch, proving all replacement/order decisions precede the single commit.
    for commander_to_command_zone in [true, false] {
        let mut e = make_engine(71 + u64::from(commander_to_command_zone), false);
        let duplicate_a = inject_library_card(&mut e, 0, "intuition_duplicate_a");
        let duplicate_b = inject_library_card(&mut e, 0, "intuition_duplicate_a");
        let distinct = inject_library_card(&mut e, 0, "intuition_distinct");
        let found = vec![duplicate_a, duplicate_b, distinct];
        let before_generations = found
            .iter()
            .map(|id| e.state.zone_change_generation.get(id).copied().unwrap_or(0))
            .collect::<Vec<_>>();
        let batch = cast_targeted_instant_and_resolve(
            &mut e,
            0,
            "intuition_comparator_fixture",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            target_player(1),
        );
        let search = find_resolution_choice(&batch).expect("exact three-card search");
        assert_eq!(search.deciding_player_id, 0);
        assert_eq!((search.min, search.max), (3, 3));
        assert!(!search.unique_names);
        assert!(search.public_reveal.is_none());
        assert!(found
            .iter()
            .all(|id| search.candidate_object_ids.contains(id)));
        let search_result = e
            .apply_scenario_command(0, &submit_resolution_choice(found.clone()))
            .expect("submit two duplicate names and the distinct card");
        let split = find_resolution_choice(&search_result).expect("opponent chooses one");
        assert_eq!(split.deciding_player_id, 1);
        assert_eq!((split.min, split.max), (1, 1));
        assert!(!split.unique_names);
        assert_eq!(split.candidate_object_ids, found);
        let reveal = split
            .public_reveal
            .clone()
            .expect("public comparator reveal");
        assert_eq!(
            search_result
                .events
                .iter()
                .filter(|event| { matches!(&event.ev, Some(Ev::CardsRevealed(_))) })
                .count(),
            1,
            "one standalone public reveal is emitted for the chosen cohort"
        );
        assert_eq!(
            search_result
                .events
                .iter()
                .find_map(|event| match &event.ev {
                    Some(Ev::CardsRevealed(receipt)) => Some(receipt),
                    _ => None,
                }),
            Some(&reveal)
        );

        e.state.players[0]
            .declared_commander_object_ids
            .extend([duplicate_a, distinct]);
        let split_batch = e
            .apply_scenario_command(1, &submit_resolution_choice(vec![distinct]))
            .expect("the target chooses the one card that goes to hand");
        let commander_choice = find_resolution_choice(&split_batch)
            .expect("a hand-bound commander asks before movement under CR 903.9b");
        assert_eq!(commander_choice.deciding_player_id, 0);
        assert!(commander_choice.prompt_text.contains("903.9b"));
        assert_eq!(commander_choice.public_reveal.as_ref(), Some(&reveal));
        assert!(found
            .iter()
            .all(|id| e.state.objects[id].zone == Zone::Library));
        assert_eq!(
            split_batch.events.iter().find_map(|event| match &event.ev {
                Some(Ev::ActivePublicRevealSnapshot(snapshot)) => snapshot
                    .reveals
                    .iter()
                    .find(|active| active.reveal_id == reveal.reveal_id),
                _ => None,
            }),
            Some(&reveal)
        );

        let mut branch = submit_resolution_choice(Vec::new());
        let Some(Cmd::SubmitResolutionChoice(answer)) = branch.cmd.as_mut() else {
            unreachable!()
        };
        answer.decision =
            tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32;
        answer.selected_branch_index = if commander_to_command_zone { 0 } else { 1 };
        let commander_batch = e
            .apply_scenario_command(0, &branch)
            .expect("the card owner chooses a commander destination");
        let order = find_resolution_choice(&commander_batch)
            .expect("the two graveyard cards receive the CR 404.3 order choice");
        assert_eq!(order.deciding_player_id, 0);
        assert_eq!(order.choice_kind(), ChoiceKind::SimultaneousEntryOrder);
        assert_eq!(order.candidate_object_ids, vec![duplicate_a, duplicate_b]);
        assert_eq!(order.public_reveal.as_ref(), Some(&reveal));
        assert!(found
            .iter()
            .all(|id| e.state.objects[id].zone == Zone::Library));
        let mut batches = vec![e
            .apply_scenario_command(0, &submit_resolution_choice(vec![duplicate_b, duplicate_a]))
            .expect("controller supplies the precommit graveyard order")];
        let sba = find_resolution_choice(&batches[0])
            .expect("the graveyard-bound commander receives 903.9a after resolution");
        assert_eq!(sba.deciding_player_id, 0);
        assert!(sba.prompt_text.contains("903.9a"));
        assert_eq!(e.state.objects[&duplicate_a].zone, Zone::Graveyard);
        assert_eq!(e.state.objects[&duplicate_b].zone, Zone::Graveyard);
        let mut leave_commander = submit_resolution_choice(Vec::new());
        let Some(Cmd::SubmitResolutionChoice(answer)) = leave_commander.cmd.as_mut() else {
            unreachable!()
        };
        answer.decision =
            tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32;
        answer.selected_branch_index = 1;
        batches.push(
            e.apply_scenario_command(0, &leave_commander)
                .expect("CR 903.9a owner leaves the commander in their graveyard"),
        );
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(e.state.objects[&duplicate_a].zone, Zone::Graveyard);
        assert_eq!(e.state.objects[&duplicate_b].zone, Zone::Graveyard);
        assert_eq!(
            e.state.objects[&distinct].zone,
            if commander_to_command_zone {
                Zone::Command
            } else {
                Zone::Hand
            }
        );
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(
                    &event.ev,
                    Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
                ))
                .count(),
            1
        );
        for (id, before) in found.iter().zip(before_generations) {
            assert_eq!(
                e.state.zone_change_generation.get(id).copied().unwrap_or(0),
                before + 1,
                "each member of the Intuition partition has one committed zone transition"
            );
        }
    }

    // The Intuition template asks for three cards (bounds 3/3) without a DifferentNames rule.
    // With only two physical copies of the same name in P0's library, the runtime clamps the
    // minimum to two but retains the authored maximum of three; both candidates still must be used.
    let mut e = make_engine(79, true);
    assert_eq!(e.state.players[0].library.len(), 2, "short-library fixture");
    let short = e.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let before_generations = short
        .iter()
        .map(|id| e.state.zone_change_generation.get(id).copied().unwrap_or(0))
        .collect::<Vec<_>>();
    for id in &short {
        e.state.objects.get_mut(id).unwrap().card_id = "intuition_duplicate_a".into();
    }
    let batch = cast_targeted_instant_and_resolve(
        &mut e,
        0,
        "intuition_comparator_fixture",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_player(1),
    );
    let search = find_resolution_choice(&batch).expect("clamped short-library search");
    assert_eq!((search.min, search.max), (2, 3));
    assert_eq!(search.candidate_object_ids, short);
    assert!(!search.unique_names);
    assert!(
        e.apply_scenario_command(0, &submit_resolution_choice(vec![short[0]]))
            .is_err(),
        "the minimum still requires both available cards"
    );
    assert!(
        e.state.pending_resolution.is_some(),
        "search remains pending after under-minimum submission"
    );
    let after_search = e
        .apply_scenario_command(0, &submit_resolution_choice(short.clone()))
        .expect("submit all available library cards");
    let split = find_resolution_choice(&after_search).expect("target chooses from two cards");
    assert_eq!(split.deciding_player_id, 1);
    assert_eq!((split.min, split.max), (1, 1));
    assert!(!split.unique_names);
    assert_eq!(split.candidate_object_ids, short);
    let chosen = split.candidate_object_ids[0];
    let moved = e
        .apply_scenario_command(1, &submit_resolution_choice(vec![chosen]))
        .expect("target puts one card in hand; the remainder enters graveyard");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.objects[&chosen].zone, Zone::Hand);
    let remainder = *short.iter().find(|id| **id != chosen).expect("remainder");
    assert_eq!(e.state.objects[&remainder].zone, Zone::Graveyard);
    for (id, before) in short.iter().zip(before_generations) {
        assert_eq!(e.state.zone_change_generation[id], before + 1);
    }
    assert_eq!(
        moved
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
            ))
            .count(),
        1
    );
}

/// Gifts Ungiven: submitting two library cards with the same name must be rejected (Oracle:
/// "up to four cards with *different* names"). The engine enforces this at choice submission
/// via `PendingResolution::unique_names`; the pending state must be restored on failure.
#[test]
fn gifts_ungiven_rejects_same_name_in_search() {
    // Minimal deck; we inject the cards we need deterministically after setup so shuffle
    // order cannot affect which cards are in the library when the search fires.
    let decks = Some(vec![
        {
            let mut d = vec!["gifts_ungiven".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        8,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject two Lightning Bolts (same name, different objects) plus three distinct others so
    // the test has 2 duplicate-name candidates and ≥4 distinct-name candidates available.
    let bolt1_id = inject_library_card(&mut e, 0, "lightning_bolt");
    let bolt2_id = inject_library_card(&mut e, 0, "lightning_bolt");
    let div_id = inject_library_card(&mut e, 0, "divination");
    let ctsp_id = inject_library_card(&mut e, 0, "counterspell");
    let murd_id = inject_library_card(&mut e, 0, "murder");

    let batch = cast_targeted_instant_and_resolve(
        &mut e,
        0,
        "gifts_ungiven",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_player(1),
    );

    let search = find_resolution_choice(&batch).expect("search choice");
    assert_eq!(search.deciding_player_id, 0);

    // bolt1 and bolt2 are both valid candidates (they are in the library).
    assert!(
        search.candidate_object_ids.contains(&bolt1_id),
        "bolt1 must be in candidates"
    );
    assert!(
        search.candidate_object_ids.contains(&bolt2_id),
        "bolt2 must be in candidates"
    );

    // Illegal: two cards with the same name (both Lightning Bolts) plus one other.
    let bad_choice = vec![bolt1_id, bolt2_id, div_id];
    let err = e
        .apply_scenario_command(0, &submit_resolution_choice(bad_choice))
        .expect_err("same-name cards must be rejected");
    assert!(
        matches!(err, tricerules_core::EngineError::Illegal(_)),
        "expected Illegal, got {err:?}"
    );

    // Pending resolution must still be set (state rolled back) so the player can retry.
    assert!(
        e.state.pending_resolution.is_some(),
        "pending resolution must be restored after rejection"
    );

    // Happy path: submit 4 cards with all different names — should succeed.
    // bolt1 (Lightning Bolt) + divination + counterspell + murder
    let good_choice = vec![bolt1_id, div_id, ctsp_id, murd_id];
    e.apply_scenario_command(0, &submit_resolution_choice(good_choice))
        .expect("four different-name cards must be accepted");

    // Resolution now needs the opponent to choose which 2 go to graveyard.
    assert!(
        e.state.pending_resolution.is_some(),
        "opponent split still pending"
    );
}

#[test]
fn gifts_different_names_search_rejects_duplicate_split_and_room_physical_copies() {
    // Each pair has two distinct physical objects with the same legal multiface definition.
    // This reaches Gifts' actual DifferentNames search validator while keeping the synthetic
    // registry's deck aliases valid; names_outside_stack supplies both face names for each copy.
    let split = r#"(
        id: "layout_overlap_split",
        name: "First Flame // Split Frost",
        layout: Split,
        faces: [
            (name: "First Flame", face_id: "first_flame", mana_cost: "{0}", types: ["Sorcery"]),
            (name: "Split Frost", face_id: "split_frost", mana_cost: "{0}", types: ["Sorcery"]),
        ],
    )"#;
    let room = r#"(
        id: "layout_overlap_room",
        name: "North Room // South Room",
        layout: Room,
        faces: [
            (name: "North Room", face_id: "north_room", mana_cost: "{0}", types: ["Enchantment", "Room"]),
            (name: "South Room", face_id: "south_room", mana_cost: "{0}", types: ["Enchantment", "Room"]),
        ],
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/gifts_ungiven.ron"),
                split,
                room,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("test-only Split/Room definitions and Gifts validate"),
    ));
    let decks = Some(vec![
        {
            let mut deck = vec!["gifts_ungiven".to_string()];
            deck.extend(std::iter::repeat_n("island".to_string(), 29));
            deck
        },
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(registry, 8_701, &[0, 1], 20, decks, true)
        .expect("new test-only layout-overlap fixture");
    advance_to_main1_from_game_start(&mut engine);
    let split_id = inject_library_card(&mut engine, 0, "layout_overlap_split");
    let split_copy_id = inject_library_card(&mut engine, 0, "layout_overlap_split");
    let room_id = inject_library_card(&mut engine, 0, "layout_overlap_room");
    let room_copy_id = inject_library_card(&mut engine, 0, "layout_overlap_room");
    let cast = cast_targeted_instant_and_resolve(
        &mut engine,
        0,
        "gifts_ungiven",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_player(1),
    );
    let search = find_resolution_choice(&cast).expect("actual Gifts search offer");
    assert!(search.unique_names);
    for object_id in [split_id, split_copy_id, room_id, room_copy_id] {
        assert!(search.candidate_object_ids.contains(&object_id));
    }
    let mut duplicate_split_copies = submit_resolution_choice(vec![split_id, split_copy_id]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = duplicate_split_copies.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = search.resolution_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut engine,
        0,
        &duplicate_split_copies,
        "two physical Split cards with overlapping face-name sets",
    );
    let mut duplicate_room_copies = submit_resolution_choice(vec![room_id, room_copy_id]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = duplicate_room_copies.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = search.resolution_prompt_id;
    assert_rejected_gifts_answer_is_immutable(
        &mut engine,
        0,
        &duplicate_room_copies,
        "two physical Room cards with overlapping face-name sets",
    );
}

/// The ruled runtime rejects legacy `custom_effect` syntax before moving or resolving a spell.
/// Keep current card data free of that retired path; new mechanics belong in typed programs or
/// engine primitives so their parked state is replayable and validated centrally.
#[test]
fn card_registry_has_no_legacy_custom_effect_owners() {
    for definition in tricerules_cards::registry::global().definitions() {
        for face in definition.faces_iter() {
            assert!(
                face.custom_effect.is_none(),
                "{}:{} still uses unsupported custom_effect syntax",
                definition.id,
                face.face_id
            );
        }
    }
}

// ── Summoning sickness on re-entry ─────────────────────────────────────────────

#[test]
fn recast_bounced_creature_is_summoning_sick() {
    // CR 302.6: a creature that left and re-entered the battlefield has not been controlled
    // continuously since its controller's turn began, so it is summoning sick again. Regression:
    // bouncing a creature cleared `summoning_sick`, and entering the battlefield never re-asserted
    // it, so a bounced-and-recast creature could attack/tap the same turn. Entry now re-asserts it.
    use tricerules_core::Zone;
    let decks = Some(vec![
        {
            let mut d = vec!["grizzly_bears".to_string(), "unsummon".to_string()];
            d.extend(std::iter::repeat_n("forest".to_string(), 20));
            d.extend(std::iter::repeat_n("island".to_string(), 8));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4242,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);

    // A grizzly already established on the battlefield (no longer summoning sick).
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    let grizzly = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(
        !e.state.objects[&grizzly].summoning_sick,
        "an established creature is not summoning sick"
    );

    // Bounce it back to hand with Unsummon ({U}).
    ensure_in_hand(&mut e, 0, "unsummon");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon_idx = hand_index_for_card(&e, 0, "unsummon");
    e.apply_scenario_command(
        0,
        &cast_spell(
            unsummon_idx,
            vec![TargetRef {
                expected_zone_change_generation: None,
                object_id: grizzly,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast unsummon at own grizzly");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&grizzly].zone,
        Zone::Hand,
        "grizzly bounced to hand"
    );

    // Recast the same grizzly ({1}{G}). The object id is reused across the zone change.
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let grizzly_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(grizzly_idx, vec![]))
        .expect("recast grizzly");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&grizzly].zone,
        Zone::Battlefield,
        "grizzly recast onto the battlefield"
    );
    assert!(
        e.state.objects[&grizzly].summoning_sick,
        "the recast creature is summoning sick again (CR 302.6)"
    );
}
