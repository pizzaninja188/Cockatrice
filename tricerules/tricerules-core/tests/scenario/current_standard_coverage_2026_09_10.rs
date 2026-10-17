use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand,
    SubmitResolutionChoice,
};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn setup_hand_bottom_spell(card_id: &str, seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[card_id]), forest_only_deck()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("registered hand-bottom spell fixture");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, card_id);
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
}

fn resolve_top_spell(engine: &mut GameEngine) {
    for _ in 0..engine.state.players.len() {
        let player = engine.state.priority_player_id();
        engine
            .apply_scenario_command(player, &pass())
            .expect("pass priority to resolve the spell");
    }
}

#[test]
fn heated_argument_is_registered_in_the_current_standard_audit() {
    let definition = tricerules_cards::registry::global()
        .get("heated_argument")
        .expect("Heated Argument is in the current-Standard coverage cohort");
    assert_eq!(definition.name, "Heated Argument");
    assert_eq!(definition.primary_face().spell_effect.len(), 2);
}

#[test]
fn hand_bottom_damage_spells_share_the_resolution_cost_primitive() {
    for (card_id, target_kind_count) in [
        ("manhole_missile", 1),
        ("fire_prophecy", 1),
        ("volcanic_spite", 3),
    ] {
        let definition = tricerules_cards::registry::global()
            .get(card_id)
            .unwrap_or_else(|| panic!("{card_id} is registered"));
        let face = definition.primary_face();
        assert!(
            face.spell_effect.is_empty(),
            "{card_id} moved to its Program body"
        );
        let owner = tricerules_cards::ProgramOwnerId::face(
            card_id,
            tricerules_cards::CardFaceId::new(card_id).unwrap(),
        );
        let program = tricerules_cards::registry::global()
            .compiled_program_for_owner(&owner)
            .unwrap_or_else(|| panic!("{card_id} has its owner-local Program"));
        assert_eq!(
            program.target_bindings.len(),
            1,
            "{card_id} binds its damage target"
        );
        assert_eq!(program.target_bindings[0].target_group_index, 0);
        assert_eq!(program.target_bindings[0].role_index, 0);
        let [damage, choose] = program.body.steps.as_slice() else {
            panic!("{card_id} retains damage followed by the optional payment");
        };
        let tricerules_cards::primitives::CompiledInstructionKind::Perform(damage) = &damage.kind
        else {
            panic!("{card_id} starts with targeted damage");
        };
        let tricerules_cards::primitives::SpellEffectKind::DamageTarget { amount, target } =
            damage.as_ref()
        else {
            panic!("{card_id} starts with targeted damage");
        };
        assert_eq!(*amount, tricerules_cards::primitives::Amount::Fixed(3));
        let leaves = target.any_of.as_ref().map_or(1, Vec::len);
        assert_eq!(leaves, target_kind_count);
        let tricerules_cards::primitives::CompiledInstructionKind::Choose {
            chooser,
            optional,
            branches,
            otherwise,
            ..
        } = &choose.kind
        else {
            panic!("{card_id} ends with an optional paid branch");
        };
        assert_eq!(
            *chooser,
            tricerules_cards::primitives::PlayerRecipient::Controller
        );
        assert!(*optional);
        assert!(otherwise.steps.is_empty());
        assert_eq!(branches.len(), 1);
        assert_eq!(
            branches[0].cost,
            tricerules_cards::primitives::ResolutionCost::PutHandCardOnLibraryBottom
        );
        assert!(matches!(
            branches[0].body.steps.as_slice(),
            [draw] if matches!(
                &draw.kind,
                tricerules_cards::primitives::CompiledInstructionKind::Perform(effect)
                    if matches!(effect.as_ref(), tricerules_cards::primitives::SpellEffectKind::Draw {
                        who: tricerules_cards::primitives::PlayerRecipient::Controller,
                        count: tricerules_cards::primitives::Amount::Fixed(1),
                    })
            )
        ));
    }
}

#[test]
fn fire_prophecy_runs_its_paid_branch_and_rejects_noncreature_targets() {
    let mut engine = setup_hand_bottom_spell("fire_prophecy", 910_101);
    let creature = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    let spell_slot = hand_index_for_card(&engine, 0, "fire_prophecy");
    engine
        .apply_scenario_command(0, &cast_spell(spell_slot, target_object(creature)))
        .expect("Fire Prophecy legally targets a creature");
    resolve_top_spell(&mut engine);
    assert_eq!(engine.state.objects[&creature].damage, 3);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose Fire Prophecy's optional payment");
    let card = *engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .iter()
        .find(|oid| engine.state.objects[oid].card_id == "mountain")
        .expect("a Mountain is available as the exact hand-card payment");
    let drawn = *engine.state.players[0]
        .library
        .front()
        .expect("Fire Prophecy can draw from the library");
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![card]))
        .expect("pay Fire Prophecy's hand-bottom cost");
    assert_eq!(engine.state.objects[&card].zone, Zone::Library);
    assert_eq!(engine.state.players[0].library.back(), Some(&card));
    assert!(engine.state.players[0].hand.contains(&drawn));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    for (seed, target_id) in [
        (910_102, "jace_beleren"),
        (910_103, "invasion_of_ulgrotha_grandmother_ravi_sengir"),
    ] {
        let mut invalid = setup_hand_bottom_spell("fire_prophecy", seed);
        let target = inject_permanent_on_battlefield(&mut invalid, 1, target_id);
        let spell_slot = hand_index_for_card(&invalid, 0, "fire_prophecy");
        let before = serde_json::to_value(&invalid.state).unwrap();
        assert!(
            invalid
                .apply_scenario_command(0, &cast_spell(spell_slot, target_object(target)))
                .is_err(),
            "Fire Prophecy must reject {target_id}"
        );
        assert_eq!(serde_json::to_value(&invalid.state).unwrap(), before);
    }
}

#[test]
fn volcanic_spite_accepts_each_oracle_target_type_and_fizzle_skips_payment() {
    for (seed, target_kind, target_id) in [
        (910_111, "creature", "indomitable_ancients"),
        (910_112, "planeswalker", "jace_beleren"),
        (
            910_113,
            "battle",
            "invasion_of_ulgrotha_grandmother_ravi_sengir",
        ),
    ] {
        let mut engine = setup_hand_bottom_spell("volcanic_spite", seed);
        let target = if target_kind == "creature" {
            inject_creature_with_stats(&mut engine, 1, target_id, 2, 10)
        } else {
            inject_permanent_on_battlefield(&mut engine, 1, target_id)
        };
        if target_kind == "planeswalker" {
            engine
                .state
                .objects
                .get_mut(&target)
                .unwrap()
                .set_counter(CounterKind::Loyalty, 5);
        } else if target_kind == "battle" {
            engine
                .state
                .objects
                .get_mut(&target)
                .unwrap()
                .set_counter(CounterKind::Defense, 5);
        }
        let spell_slot = hand_index_for_card(&engine, 0, "volcanic_spite");
        engine
            .apply_scenario_command(0, &cast_spell(spell_slot, target_object(target)))
            .unwrap_or_else(|_| panic!("Volcanic Spite legally targets a {target_kind}"));
        resolve_top_spell(&mut engine);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| pending.presentation.choice_kind),
            Some(tricerules_proto::ruled::v1::ChoiceKind::ResolutionBranch),
            "the {target_kind} target resolves through damage before its optional branch; stack={:?}",
            engine.state.stack
        );
        if target_kind == "creature" {
            assert_eq!(engine.state.objects[&target].damage, 3);
            engine
                .apply_scenario_command(0, &select_branch(0))
                .expect("choose the optional branch for the creature target");
            let card = *engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates
                .iter()
                .find(|oid| engine.state.objects[oid].card_id == "mountain")
                .expect("a Mountain is available for payment");
            engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![card]))
                .expect("pay Volcanic Spite's hand-bottom cost");
            assert_eq!(engine.state.objects[&card].zone, Zone::Library);
        } else {
            engine
                .apply_scenario_command(
                    0,
                    &submit_resolution_decision(ResolutionChoiceDecision::Decline),
                )
                .expect("decline the optional branch");
        }
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }

    let mut fizzled = setup_hand_bottom_spell("volcanic_spite", 910_114);
    let target = inject_creature_with_stats(&mut fizzled, 1, "indomitable_ancients", 2, 10);
    let slot = hand_index_for_card(&fizzled, 0, "volcanic_spite");
    fizzled
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Volcanic Spite at the creature");
    fizzled.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    fizzled.state.players[1].graveyard.push(target);
    fizzled.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *fizzled
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    let hand_before = fizzled.state.players[0].hand.clone();
    let library_before = fizzled.state.players[0].library.clone();
    resolve_top_spell(&mut fizzled);
    assert!(fizzled.state.pending_resolution.is_none());
    assert_eq!(fizzled.state.players[0].hand, hand_before);
    assert_eq!(fizzled.state.players[0].library, library_before);
    assert!(fizzled.state.stack.is_empty());
}

#[test]
fn environmental_scientist_search_is_optional_private_and_basic_only() {
    let decks = Some(vec![
        deck_with("forest", &["environmental_scientist"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        910_001,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "environmental_scientist");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "environmental_scientist");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Environmental Scientist");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    let branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("optional search branch");
    assert!(branch.presentation.candidates.is_empty());
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose to search");
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("private library choice");
    assert!(!choice.presentation.candidates.is_empty());
    assert!(choice
        .presentation
        .candidates
        .iter()
        .all(|oid| engine.state.objects[oid].card_id == "forest"));
    let forest = choice.presentation.candidates[0];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose basic land");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Hand);
}

#[test]
fn environmental_scientist_trigger_keeps_its_search_after_the_source_leaves() {
    let decks = Some(vec![
        deck_with("forest", &["environmental_scientist"]),
        deck_with("mountain", &["lightning_bolt"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        910_003,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let forest = inject_library_card(&mut engine, 0, "forest");
    ensure_in_hand(&mut engine, 0, "environmental_scientist");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let scientist_slot = hand_index_for_card(&engine, 0, "environmental_scientist");
    engine
        .apply_scenario_command(0, &cast_spell(scientist_slot, vec![]))
        .expect("cast Environmental Scientist");
    pass_both_players(&mut engine);
    let scientist = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "environmental_scientist")
        .expect("Environmental Scientist resolved onto the battlefield");

    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes with the entry trigger on the stack");
    let bolt_slot = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_scenario_command(1, &cast_spell(bolt_slot, target_object(scientist)))
        .expect("opponent removes the trigger's source");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&scientist].zone, Zone::Graveyard);

    let branch_batch = pass_both_players_with_batch(&mut engine);
    let branch = find_resolution_choice(&branch_batch)
        .expect("the independent trigger retains its optional search");
    assert_eq!(branch.deciding_player_id, 0);
    let search_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the search after the source leaves");
    let search = find_resolution_choice(&search_batch).expect("private basic-land search");
    assert!(search.candidate_object_ids.contains(&forest));
    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose the exact Forest");

    assert_eq!(engine.state.objects[&forest].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&forest));
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == forest)
    )));
    assert!(completion.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    ));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn hire_a_crew_pumps_its_token_and_only_controlled_creatures() {
    let decks = Some(vec![
        deck_with("mountain", &["hire_a_crew", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        910_002,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let friendly = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let opposing = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "hire_a_crew");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "hire_a_crew");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Hire a Crew");
    resolve_entire_stack_two_player(&mut engine);

    let villains = battlefield_token_oids(&engine, 0, "villain_b_2_1_menace");
    let [villain] = villains.as_slice() else {
        panic!("Hire a Crew should create one Villain");
    };
    assert_eq!(engine.effective_power(*villain), Some(3));
    assert!(engine.effective_has_keyword(*villain, Keyword::Menace));
    assert_eq!(engine.effective_power(friendly), Some(3));
    assert_eq!(engine.effective_power(opposing), Some(2));
}

#[test]
fn front_porch_sentries_rejects_friendly_target_and_uses_death_lki() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &["front_porch_sentries", "murder", "grizzly_bears"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        910_003,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let sentries = relocate_to_battlefield(&mut engine, 0, "front_porch_sentries", false);
    let friendly = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let opposing = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "murder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(sentries)))
        .expect("cast Murder");
    engine
        .apply_scenario_command(0, &pass())
        .expect("caster passes");
    engine
        .apply_scenario_command(1, &pass())
        .expect("Murder resolves");
    assert_eq!(engine.state.objects[&sentries].zone, Zone::Graveyard);
    assert!(engine
        .apply_scenario_command(0, &choose_trigger_target(friendly))
        .is_err());
    engine
        .apply_scenario_command(0, &choose_trigger_target(opposing))
        .expect("choose opposing creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(opposing), Some(1));
    assert_eq!(engine.effective_toughness(opposing), Some(1));
}

#[test]
fn mountain_kings_return_recruit_creates_a_token_only_for_a_nonland_discard() {
    fn resolve(discard_nonland: bool) -> GameEngine {
        let decks = Some(vec![
            deck_with("plains", &["the_mountain-kings_return", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            910_010 + u64::from(discard_nonland),
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let discard_id = if discard_nonland {
            "grizzly_bears"
        } else {
            "plains"
        };
        ensure_in_hand(&mut engine, 0, discard_id);
        let discard_oid = engine.state.players[0].hand[hand_index_for_card(&engine, 0, discard_id)];
        let library_before = engine.state.players[0].library.len();
        let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
        let saga_was_in_library = engine.state.players[0].library.iter().any(|oid| {
            engine
                .state
                .objects
                .get(oid)
                .is_some_and(|object| object.card_id == "the_mountain-kings_return")
        });
        move_ready_to_battlefield(&mut engine, 0, "the_mountain-kings_return");
        pass_both_players(&mut engine);
        let choice = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Recruit discard choice");
        assert_eq!((choice.presentation.min, choice.presentation.max), (1, 1));
        assert!(choice.presentation.candidates.contains(&discard_oid));
        assert_eq!(
            engine.state.players[0].library.len(),
            library_before - 1 - usize::from(saga_was_in_library)
        );
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before + 1
        );
        engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![discard_oid]))
            .expect("discard for Recruit");
        assert_eq!(engine.state.objects[&discard_oid].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.players[0].library.len(),
            library_before - 1 - usize::from(saga_was_in_library)
        );
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before + 1
        );
        engine
    }

    assert_eq!(
        battlefield_token_oids(&resolve(true), 0, "human_soldier_w_1_1").len(),
        1
    );
    assert!(battlefield_token_oids(&resolve(false), 0, "human_soldier_w_1_1").is_empty());
}

#[test]
fn mountain_kings_return_chapter_two_returns_only_its_owners_small_creature_cards() {
    let decks = Some(vec![
        deck_with("plains", &["the_mountain-kings_return"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        910_012,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let saga = relocate_to_battlefield(&mut engine, 0, "the_mountain-kings_return", false);
    engine
        .state
        .objects
        .get_mut(&saga)
        .expect("Mountain-King's Return")
        .set_counter(tricerules_cards::primitives::CounterKind::Lore, 1);
    let own_small_creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let own_large_creature = inject_graveyard_card(&mut engine, 0, "serra_angel");
    let own_noncreature = inject_graveyard_card(&mut engine, 0, "bonesplitter");
    let opponent_small_creature = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    engine.state.turn_step = tricerules_core::TurnStep::Draw;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&saga].counter_count(tricerules_cards::primitives::CounterKind::Lore),
        2
    );
    assert!(engine.state.pending_triggers.iter().any(|pending| {
        pending.ability.trigger
            == tricerules_cards::primitives::TriggerCondition::SagaChapter { chapters: vec![2] }
    }));
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(own_small_creature),
                })),
            },
        )
        .expect("confirm Chapter II has no targets");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&own_small_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&own_large_creature].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&own_noncreature].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&opponent_small_creature].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&saga].zone, Zone::Battlefield);
}
