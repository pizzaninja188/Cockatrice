use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use prost::Message;
use std::collections::BTreeSet;

fn legacy_effects_contain_draw_discard(
    effects: &[tricerules_card_model::primitives::SpellEffectKind],
) -> bool {
    fn contains(effect: &tricerules_card_model::primitives::SpellEffectKind) -> bool {
        use tricerules_card_model::primitives::{ResolvingPermanentModifier, SpellEffectKind};
        match effect {
            SpellEffectKind::DrawDiscard { .. } => true,
            SpellEffectKind::Conditional { effect, .. }
            | SpellEffectKind::ConditionalCastCost { effect, .. } => contains(effect),
            SpellEffectKind::ChooseResolutionBranch {
                branches,
                otherwise,
                ..
            } => branches
                .iter()
                .flat_map(|branch| &branch.effects)
                .chain(otherwise)
                .any(contains),
            SpellEffectKind::CreateReflexiveTrigger { ability, .. } => {
                ability.effect.iter().any(contains)
            }
            SpellEffectKind::GrantTriggeredAbility { ability, .. }
            | SpellEffectKind::CreateDelayedTrigger { ability, .. } => {
                ability.effect.iter().any(contains)
            }
            SpellEffectKind::ApplyPermanentModifier {
                modifier: ResolvingPermanentModifier::GrantActivatedAbility(ability),
                ..
            } => ability.effect.iter().any(contains),
            _ => false,
        }
    }

    effects.iter().any(contains)
}

fn insert_legacy_activated_owner(
    owners: &mut BTreeSet<tricerules_card_model::ProgramOwnerId>,
    card_id: &str,
    face_id: &tricerules_card_model::CardFaceId,
    ability_path: Vec<tricerules_card_model::AbilityId>,
    ability: &tricerules_card_model::ActivatedAbilityDef,
) {
    if ability.program.is_none() && legacy_effects_contain_draw_discard(&ability.effect) {
        owners.insert(tricerules_card_model::ProgramOwnerId::ability(
            tricerules_card_model::AbilityDefinitionId {
                card_id: card_id.to_owned(),
                face_id: face_id.clone(),
                ability_path,
            },
        ));
    }
}

fn insert_legacy_triggered_owner(
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
        if modal.modes.iter().any(|mode| {
            mode.program.is_some() || legacy_effects_contain_draw_discard(&mode.effects)
        }) {
            for mode in modal.modes.iter().filter(|mode| mode.program.is_none()) {
                owners.insert(tricerules_card_model::ProgramOwnerId::ability_mode(
                    definition.clone(),
                    mode.mode_id.clone(),
                ));
            }
        }
    } else if ability.program.is_none() && legacy_effects_contain_draw_discard(&ability.effect) {
        owners.insert(tricerules_card_model::ProgramOwnerId::ability(definition));
    }
}

fn embedded_draw_discard_source_owners() -> BTreeSet<tricerules_card_model::ProgramOwnerId> {
    use tricerules_card_model::primitives::StaticAbilityDef;
    let registry = tricerules_cards::registry::global();
    let mut owners = BTreeSet::new();
    for card in registry.definitions() {
        for face in &card.faces {
            if face.spell_program.is_none()
                && legacy_effects_contain_draw_discard(&face.spell_effect)
            {
                owners.insert(tricerules_card_model::ProgramOwnerId::face(
                    card.id.clone(),
                    face.face_id.clone(),
                ));
            }
            if let Some(modal) = &face.modal_spell {
                if modal.modes.iter().any(|mode| {
                    mode.program.is_some() || legacy_effects_contain_draw_discard(&mode.effects)
                }) {
                    for mode in modal.modes.iter().filter(|mode| mode.program.is_none()) {
                        owners.insert(tricerules_card_model::ProgramOwnerId::face_mode(
                            card.id.clone(),
                            face.face_id.clone(),
                            mode.mode_id.clone(),
                        ));
                    }
                }
            }

            for ability in &face.activated_abilities {
                insert_legacy_activated_owner(
                    &mut owners,
                    &card.id,
                    &face.face_id,
                    vec![ability.ability_id.clone()],
                    ability,
                );
            }
            for ability in &face.triggered_abilities {
                insert_legacy_triggered_owner(
                    &mut owners,
                    &card.id,
                    &face.face_id,
                    vec![ability.ability_id.clone()],
                    ability,
                );
            }
            for bar in &face.class_level_bars {
                insert_legacy_activated_owner(
                    &mut owners,
                    &card.id,
                    &face.face_id,
                    vec![bar.level_ability.ability_id.clone()],
                    &bar.level_ability,
                );
                for ability in &bar.activated_abilities {
                    insert_legacy_activated_owner(
                        &mut owners,
                        &card.id,
                        &face.face_id,
                        vec![ability.ability_id.clone()],
                        ability,
                    );
                }
                for ability in &bar.triggered_abilities {
                    insert_legacy_triggered_owner(
                        &mut owners,
                        &card.id,
                        &face.face_id,
                        vec![ability.ability_id.clone()],
                        ability,
                    );
                }
            }

            for parent in face.static_abilities.iter().chain(
                face.class_level_bars
                    .iter()
                    .flat_map(|bar| &bar.static_abilities),
            ) {
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
                    insert_legacy_activated_owner(
                        &mut owners,
                        &card.id,
                        &face.face_id,
                        vec![parent_id.clone(), ability.ability_id.clone()],
                        ability,
                    );
                }
                for ability in triggered {
                    insert_legacy_triggered_owner(
                        &mut owners,
                        &card.id,
                        &face.face_id,
                        vec![parent_id.clone(), ability.ability_id.clone()],
                        ability,
                    );
                }
            }
        }
    }
    owners
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

fn draw_discard_owner_label(owner: &tricerules_card_model::ProgramOwnerId) -> String {
    use tricerules_card_model::ProgramOwnerId;

    let face_label = |face: &tricerules_card_model::CardFaceDefinitionId| {
        format!("{}/{}", face.card_id, face.face_id)
    };
    let ability_label = |ability: &tricerules_card_model::AbilityDefinitionId| {
        format!(
            "{}/{}:{}",
            ability.card_id,
            ability.face_id,
            ability
                .ability_path
                .iter()
                .map(tricerules_card_model::AbilityId::as_str)
                .collect::<Vec<_>>()
                .join("/")
        )
    };

    match owner {
        ProgramOwnerId::Face(face) => format!("face:{}", face_label(face)),
        ProgramOwnerId::FaceMode { face, mode_id } => {
            format!("face-mode:{}:{mode_id}", face_label(face))
        }
        ProgramOwnerId::Ability(ability) => format!("ability:{}", ability_label(ability)),
        ProgramOwnerId::AbilityMode { ability, mode_id } => {
            format!("ability-mode:{}:{mode_id}", ability_label(ability))
        }
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
fn embedded_draw_discard_source_owner_census_is_complete() {
    let registry = tricerules_cards::registry::global();
    let legacy_owners = embedded_draw_discard_source_owners();
    let frozen_owners = frozen_draw_discard_source_owners();
    let source_authored_owners = source_authored_program_owners();
    let authored_owners = frozen_owners
        .intersection(&source_authored_owners)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert!(
        legacy_owners.is_disjoint(&authored_owners),
        "converted authored owners must not remain in the legacy source census"
    );
    let owners = legacy_owners
        .union(&authored_owners)
        .cloned()
        .collect::<BTreeSet<_>>();
    let labels = |owners: &BTreeSet<tricerules_card_model::ProgramOwnerId>| {
        owners
            .iter()
            .map(draw_discard_owner_label)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        owners,
        frozen_owners,
        "the current legacy/authored partition must exactly cover the frozen baseline cohort; current={:?}, frozen={:?}",
        labels(&owners),
        labels(&frozen_owners)
    );
    for owner in &owners {
        let program = registry
            .compiled_program_for_owner(owner)
            .unwrap_or_else(|| panic!("missing compiled program for exact source owner {owner:?}"));
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
        !matches!(
            &pending.continuation,
            ResolutionContinuation::Custom { key, .. } if key == "brainstorm"
        ),
        "Brainstorm must suspend through the standard program runner, not its per-card custom path"
    );
    // Cast removed brainstorm from hand; begin drew three.
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1 + 3);
    assert_eq!(e.state.players[0].library.len(), lib_before - 3);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        3,
        "custom draws report the same committed draw history as primitive draws"
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
fn source_lowered_soft_counter_resumes_at_its_following_discard_after_payment() {
    let counter = r#"(
        id: "program_counter_then_loot",
        name: "Program Counter Then Loot",
        face_id: "program_counter_then_loot",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [
            CounterTargetSpell(spell_filter: (), unless_controller_pays: Some(1)),
            DrawDiscard(who: Controller, draw_count: 1, discard_count: 1,
                order: DrawThenDiscard, optional: false)
        ],
        targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target spell",
            effect_indices: [0])]))
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
        .expect("the source-lowered instruction after the payment must resume");
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
        .expect("finish the resumed draw-discard instruction");
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
    assert_eq!(engine.state.players[1].life, 20,
        "the nested ControllerLibraryEmpty no-arm receives only group 0; group 1 was still announced")
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
fn source_lowered_soft_counter_resumes_at_its_following_discard_after_declined_payment() {
    let counter = r#"(
        id: "program_counter_then_loot_decline",
        name: "Program Counter Then Loot Decline",
        face_id: "program_counter_then_loot_decline",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [
            CounterTargetSpell(spell_filter: (), unless_controller_pays: Some(1)),
            DrawDiscard(who: Controller, draw_count: 1, discard_count: 1,
                order: DrawThenDiscard, optional: false)
        ],
        targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target spell",
            effect_indices: [0])]))
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
        .expect("the source-lowered instruction after a declined payment must resume");
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
        .expect("finish the resumed draw-discard instruction");
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
fn source_lowered_copy_tokens_resume_after_entry_counter_replacement_and_order_choices() {
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
                    effects: [
                        DrawDiscard(who: Controller, draw_count: 1, discard_count: 1,
                            order: DrawThenDiscard, optional: false),
                        CreateTokenCopies(count: 2,
                            source: Chosen((kind: Creature, controller: You))),
                        Draw(count: 1)
                    ],
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose target creature you control", effect_indices: [1])])))
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
        .expect("cast the source-lowered token-copy program");
    pass_priority_round(&mut engine);
    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the lowered DrawDiscard chooser runs before token creation");
    assert_eq!(discard.presentation.choice_kind, ChoiceKind::HandCards);
    let discarded = discard.presentation.candidates[0];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("complete the first lowered DrawDiscard leaf");
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
        "the source-lowered program resumes after replacement and ordering choices"
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

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "gifts_ungiven",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
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
    assert_eq!(split.deciding_player_id, 1, "opponent decides the split");
    assert_eq!((split.min, split.max), (2, 2));
    assert_eq!(
        split.candidate_object_ids, found,
        "split is over the revealed set"
    );

    let to_grave: Vec<u32> = found.iter().take(2).copied().collect();
    let to_hand: Vec<u32> = found.iter().skip(2).copied().collect();
    // The controller cannot make the opponent's choice.
    assert!(
        e.apply_scenario_command(0, &submit_resolution_choice(to_grave.clone()))
            .is_err(),
        "only the deciding (opponent) player may submit"
    );
    e.apply_scenario_command(1, &submit_resolution_choice(to_grave.clone()))
        .expect("opponent submits split");

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
    assert_eq!(count_card_id_in_graveyard(&e, 0, "gifts_ungiven"), 1);
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

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "gifts_ungiven",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
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

/// Registry `custom_effect` key → the card ids claiming it, for the two directions below.
fn custom_effect_claims() -> std::collections::BTreeMap<&'static str, Vec<&'static str>> {
    let reg = tricerules_cards::registry::global();
    let mut claims: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for def in reg.definitions() {
        for face in def.faces_iter() {
            if let Some(key) = face.custom_effect.as_deref() {
                claims.entry(key).or_default().push(def.id.as_str());
            }
        }
    }
    claims
}

/// Every `custom_effect` key in the card registry must resolve to a registered `CardEffect`
/// (an unregistered key is uncastable card data), and no two cards may claim one key. The "one
/// resolution owner *per face*" rule is enforced in the cards crate; this is the core-side half:
/// the impl must exist, and it must belong to exactly one card.
///
/// Custom effects are 1:1 with card ids, like RON data cards — two cards wanting one algorithm
/// is the signal to widen a primitive, not to share an impl. Without this check, two cards
/// accidentally set to an unregistered `custom_effect` key would fail the forward registration
/// check rather than silently using another card's implementation.
/// Two faces of one card sharing a key fails too: the impl cannot tell faces apart either.
#[test]
fn every_custom_effect_key_has_exactly_one_card_and_one_impl() {
    for (key, claimants) in custom_effect_claims() {
        assert!(
            tricerules_core::custom::lookup(key).is_some(),
            "card '{}' has custom_effect '{key}' with no registered CardEffect",
            claimants[0]
        );
        assert_eq!(
            claimants.len(),
            1,
            "custom_effect '{key}' is claimed by {} cards ({}); each key belongs to exactly one \
             card. If these really share an algorithm, that algorithm is expressible as \
             (effect_kind, parameters) and belongs in a primitive.",
            claimants.len(),
            claimants.join(", ")
        );
        assert_eq!(
            claimants[0], key,
            "custom_effect '{key}' is claimed by card '{}'; the custom-effect file stem, RON \
             custom_effect, and claiming card id must all match",
            claimants[0]
        );
    }
}

/// The reverse direction: every registered impl is claimed by a card. A registered key is a file
/// stem under `src/custom/`, so an orphan means a typo'd filename or a deleted RON — neither of
/// which the forward check above can see, since it only walks keys the registry already has.
#[test]
fn every_registered_impl_is_claimed_by_a_card() {
    let claims = custom_effect_claims();
    for key in tricerules_core::custom::keys() {
        assert!(
            claims.contains_key(key),
            "custom effect `{key}` (from `src/custom/{key}.rs`) is claimed by no registry card. \
             The file stem must equal the card id whose RON sets custom_effect: \"{key}\"."
        );
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
