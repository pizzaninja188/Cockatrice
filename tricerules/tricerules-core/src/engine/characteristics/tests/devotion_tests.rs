use super::*;

fn compound_devotion_engine(support: &str, support_id: &str) -> (GameEngine, ObjectId, ObjectId) {
    let god = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tricerules-cards/data/nylea_god_of_the_hunt.ron"
    ));
    let registry = CardRegistry::from_chunks_and_tokens(&[god, support], &[]).unwrap();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        700_506,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    engine.registry = Box::leak(Box::new(registry));
    engine.state.opening = None;
    engine.state.turn_step = TurnStep::DeclareBlockers;
    let god = insert_fixture(&mut engine, 0, "nylea,_god_of_the_hunt", Zone::Battlefield);
    engine.emit_static_abilities_on_enter(god);
    let support = insert_fixture(&mut engine, 0, support_id, Zone::Battlefield);
    engine.emit_static_abilities_on_enter(support);
    assert!(engine.characteristics(god).unwrap().is_creature());
    devotion_combat(&mut engine, god, vec![]);
    (engine, god, support)
}

fn frozen_devotion_item(engine: &GameEngine, source: ObjectId, ability: &str) -> StackItem {
    let mut item = engine.observer_return_item(u32::MAX, 0);
    item.card_id = engine.state.objects[&source].card_id.clone();
    item.source_permanent_id = Some(source);
    item.source_owner = Some(0);
    item.source_zone_change = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    item.source_face_change = engine
        .state
        .face_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let fixture = format!("(id: \"frozen_devotion_frozen_devotion_back\", name: \"Frozen Devotion // Frozen Devotion Back\", layout: Transform, faces: [(name: \"Frozen Devotion\", face_id: \"frozen_devotion\", types: [\"Artifact\"], activated_abilities: [{ability}]), (name: \"Frozen Devotion Back\", face_id: \"frozen_devotion_back\", types: [\"Artifact\"])])");
    let registry = CardRegistry::from_chunks_and_tokens(&[&fixture], &[]).unwrap();
    item.activated_ability = Some(
        registry
            .get("frozen_devotion_frozen_devotion_back")
            .unwrap()
            .primary_face()
            .activated_abilities[0]
            .clone(),
    );
    item
}

fn assert_recovered_god_stays_out_of_combat(
    engine: &mut GameEngine,
    god: ObjectId,
    events: &mut Vec<rv1::RuledEvent>,
) {
    assert!(engine.characteristics(god).unwrap().is_creature());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    let combat = engine.state.combat.as_ref().unwrap().clone();
    assert!(
        combat.attacking.is_empty()
            && combat.attack_assignments.is_empty()
            && combat.blockers.is_empty()
    );
    assert!(combat.first_strike_attackers.is_empty() && combat.first_strike_blockers.is_empty());
    assert!(combat.damage_assignments.is_empty() && combat.trample_player_damage.is_empty());
    assert!(!combat.damage_assignment_needed);
    let life = engine.state.players[1].life;
    engine
        .resolve_combat_damage(
            &combat,
            super::super::super::combat::DamagePass::Normal,
            events,
        )
        .unwrap();
    assert_eq!(
        engine.state.players[1].life, life,
        "the recovered God deals no combat damage"
    );
    let removals: Vec<_> = events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::RemovedFromCombat(removed)) => {
                Some(removed.object_ids.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(removals, vec![vec![god]]);
}

#[test]
fn devotion_full_effect_list_bounces_support_then_returns_it_from_hand_without_rejoining_combat() {
    let support = r#"(id: "devotion_support", name: "Devotion Support", face_id: "devotion_support",
        mana_cost: "{G}{G}{G}{G}", types: ["Enchantment"])"#;
    let (mut engine, god, support) = compound_devotion_engine(support, "devotion_support");
    let item = frozen_devotion_item(
        &engine,
        support,
        r#"(ability_id: "compound", presentation: Fallback, costs: [],
        effect: [ReturnToOwnersHand(subject: Source), SearchLibrary(count: 1,
            filter: Some((exact_name: Some("Devotion Support"))), zones: Fixed([Hand]),
            destination: Battlefield(tapped: false), shuffle: false, reveal: false)])"#,
    );
    let (effects, label) = engine.build_resolution_effects(&item);
    let mut events = vec![];
    assert!(matches!(
        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .unwrap(),
        super::super::super::resolution::ResolutionProgress::Parked
    ));
    assert_eq!(engine.state.objects[&support].zone, Zone::Hand);
    assert!(!engine.characteristics(god).unwrap().is_creature());
    assert!(engine.state.combat.as_ref().unwrap().attacking.is_empty());
    assert!(events.iter().any(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::RemovedFromCombat(removed)) if removed.object_ids == vec![god])));
    let resumed = engine
        .apply_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![support],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    events.extend(resumed.events);
    assert_eq!(engine.state.objects[&support].zone, Zone::Battlefield);
    assert_recovered_god_stays_out_of_combat(&mut engine, god, &mut events);
}

#[test]
fn devotion_nested_transform_return_publishes_removal_before_entry_choice_and_resumes_tail() {
    let support = r#"(id: "devotion_support_front_devotion_support_back", name: "Devotion Support Front // Devotion Support Back", layout: Transform,
        faces: [(name: "Devotion Support Front", face_id: "devotion_support_front", mana_cost: "{G}{G}{G}{G}", types: ["Enchantment"]),
            (name: "Devotion Support Back", face_id: "devotion_support_back", mana_cost: "{G}{G}{G}{G}", types: ["Enchantment"],
                static_abilities: [(ability_id: "entry", presentation: Fallback,
                    definition: EntersTapped(affected: Self_, unless_cost: Some(PayLife(amount: 1))))])])"#;
    let (mut engine, god, support) =
        compound_devotion_engine(support, "devotion_support_front_devotion_support_back");
    let item = frozen_devotion_item(
        &engine,
        support,
        r#"(ability_id: "compound", presentation: Fallback, costs: [],
        effect: [ExileSourceThenReturnTransformed(), GainLife(amount: 1)])"#,
    );
    let (effects, label) = engine.build_resolution_effects(&item);
    let mut events = vec![];
    assert!(matches!(
        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .unwrap(),
        super::super::super::resolution::ResolutionProgress::Parked
    ));
    assert_eq!(engine.state.objects[&support].zone, Zone::Exile);
    assert!(!engine.characteristics(god).unwrap().is_creature());
    assert!(engine.state.combat.as_ref().unwrap().attacking.is_empty());
    assert!(events.iter().any(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::RemovedFromCombat(removed)) if removed.object_ids == vec![god])));
    assert!(engine.state.pending_replacement_event.is_some());
    let life = engine.state.players[0].life;
    let resumed = engine
        .apply_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::Decline as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    events.extend(resumed.events);
    assert_eq!(
        engine.state.players[0].life,
        life + 1,
        "the parked compound instruction resumes its effect tail"
    );
    assert_eq!(engine.state.objects[&support].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&support].face_up_index, 1);
    assert!(engine.state.objects[&support].tapped);
    assert_recovered_god_stays_out_of_combat(&mut engine, god, &mut events);
}

#[test]
fn devotion_remove_creature_retains_kindred_subtypes_and_all_creature_types() {
    for kindred in [false, true] {
        let (mut engine, god) = devotion_engine();
        let mut face = engine
            .registry
            .get("devotion_source")
            .unwrap()
            .primary_face()
            .clone();
        if kindred {
            face.types.push("Kindred".into());
        }
        engine.state.objects.get_mut(&god).unwrap().copiable_values = Some(CopiableValues {
            source_card_id: "devotion_source".into(),
            source_face_index: 0,
            display_name: face.name.clone(),
            face,
            room_faces: None,
        });
        engine.refresh_source_static_abilities(god);
        engine
            .state
            .continuous_effects
            .push(early_layer_type_effect(
                AffectedScope::Single(god),
                ContinuousEffectKind::Layer4SetAllCreatureTypes,
                0,
            ));
        for effect in &mut engine.state.continuous_effects {
            if matches!(effect.kind, ContinuousEffectKind::Layer4RemoveCreature) {
                effect.timestamp = 1;
            }
        }
        let result = engine.characteristics(god).unwrap();
        assert!(!result.is_creature());
        assert_eq!(result.has_type("God"), kindred);
        assert_eq!(result.all_creature_types, kindred);
    }
}

fn fixture_copy(engine: &mut GameEngine, oid: ObjectId, text: &str, id: &str) {
    let registry = CardRegistry::from_chunks_and_tokens(&[text], &[]).unwrap();
    let face = registry.get(id).unwrap().primary_face().clone();
    engine.state.objects.get_mut(&oid).unwrap().copiable_values = Some(CopiableValues {
        source_card_id: id.into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
}

#[test]
fn devotion_committed_cohort_registers_all_statics_before_reconciling_combat() {
    let (mut engine, god) = devotion_engine();
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    devotion_combat(&mut engine, god, vec![]);
    let first = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    fixture_copy(
        &mut engine,
        first,
        r#"(id: "cohort_replace", name: "Cohort Replace", face_id: "cohort_replace", types: ["Artifact", "Equipment"],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: AttachedModifier(set_types: Some((card_types: [Enchantment]))))])"#,
        "cohort_replace",
    );
    engine.state.objects.get_mut(&first).unwrap().attached_to =
        Some(AttachmentRecipient::Object(god));
    let second = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    fixture_copy(
        &mut engine,
        second,
        r#"(id: "cohort_restore", name: "Cohort Restore", face_id: "cohort_restore", types: ["Artifact"],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: AddTypesToPermanents(filter: (kind: AnyPermanent, permanent_types: [Enchantment]),
                addition: (card_types: [Creature])))])"#,
        "cohort_restore",
    );
    let mut events = vec![];
    engine.fire_triggers(
        &[
            GameEvent::EntersBattlefield {
                object_id: first,
                chosen_x: 0,
            },
            GameEvent::EntersBattlefield {
                object_id: second,
                chosen_x: 0,
            },
        ],
        &mut events,
    );
    assert!(engine.characteristics(god).unwrap().is_creature());
    assert_eq!(engine.state.combat.as_ref().unwrap().attacking, vec![god]);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::RemovedFromCombat(_)))),
        "partial static installation must not remove a combatant"
    );
}

#[test]
fn devotion_removed_blocker_invalidates_assignments_and_recomputes_remaining_trample_choice() {
    let mut engine = GameEngine::new_with_default_decks(
        tricerules_cards::registry::global(),
        700_505,
        &[0, 1],
        20,
    )
    .unwrap();
    let attacker = insert_fixture(&mut engine, 0, "aggressive_mammoth", Zone::Battlefield);
    let first = insert_fixture(&mut engine, 1, "grizzly_bears", Zone::Battlefield);
    let second = insert_fixture(&mut engine, 1, "grizzly_bears", Zone::Battlefield);
    devotion_combat(&mut engine, attacker, vec![first, second]);
    let mut events = vec![];
    engine.remove_combat_participants(&[first, first], &mut events);
    let combat = engine.state.combat.as_ref().unwrap();
    assert_eq!(combat.blockers[&attacker], vec![second]);
    assert_eq!(combat.first_strike_blockers[&attacker], vec![second]);
    assert!(combat.damage_assignments.is_empty() && combat.trample_player_damage.is_empty());
    assert!(
        combat.damage_assignment_needed,
        "a remaining blocker with trample requires a fresh assignment"
    );
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0].ev, Some(rv1::ruled_event::Ev::RemovedFromCombat(removed)) if removed.object_ids == vec![first])
    );
}

fn devotion_engine() -> (GameEngine, ObjectId) {
    let source = r#"(
        id: "devotion_source", name: "Devotion Source", face_id: "devotion_source",
        mana_cost: "{3}{G}", types: ["Enchantment", "Creature", "God"],
        supertypes: ["Legendary"], power: Some(6), toughness: Some(6),
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ConditionalSelfModifier(
                condition: Devotion(color: Green, max: Some(4)),
                remove_creature: true,
                add_types: (card_types: [Artifact])))],
    )"#;
    let support = r#"(
        id: "devotion_support", name: "Devotion Support", face_id: "devotion_support",
        mana_cost: "{G}{G}{G}", types: ["Enchantment"],
    )"#;
    let registry = CardRegistry::from_chunks_and_tokens(&[source, support], &[])
        .expect("devotion fixture must be admitted");
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        700_501,
        &[0, 1, 2, 3],
        20,
        None,
        true,
    )
    .unwrap();
    engine.registry = Box::leak(Box::new(registry));
    let source = insert_fixture(&mut engine, 0, "devotion_source", Zone::Battlefield);
    engine.emit_static_abilities_on_enter(source);
    (engine, source)
}

fn copy_cost(engine: &mut GameEngine, oid: ObjectId, cost: &str, token: bool) {
    let mut face = engine
        .registry
        .get("devotion_support")
        .unwrap()
        .primary_face()
        .clone();
    face.mana_cost = tricerules_card_model::ManaCost::parse(cost).unwrap();
    let values = CopiableValues {
        source_card_id: "devotion_support".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Devotion Support".into(),
    };
    let object = engine.state.objects.get_mut(&oid).unwrap();
    if token {
        object.token_origin = Some(values);
    } else {
        object.copiable_values = Some(values);
    }
}

#[test]
fn devotion_counts_each_cost_symbol_and_uses_copy_face_down_and_layer_two_control() {
    let (mut engine, source) = devotion_engine();
    assert!(engine.characteristics(source).unwrap().is_artifact());
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    assert!(
        engine.characteristics(source).unwrap().is_artifact(),
        "own pip plus three is four"
    );
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    assert!(
        !engine.characteristics(source).unwrap().is_artifact(),
        "four repeated symbols plus own pip is five"
    );
    for (cost, expected) in [
        ("{G/U}{2/G}{G/P}{G}", false),
        ("{G/G}{G}{G}", true),
        ("{9}{X}{C}{R}{W}{U}{B}{G}", true),
    ] {
        copy_cost(&mut engine, support, cost, false);
        assert_eq!(
            engine.characteristics(source).unwrap().is_artifact(),
            expected,
            "{cost}"
        );
    }
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    engine.state.objects.get_mut(&support).unwrap().face_down = true;
    assert!(
        engine.characteristics(source).unwrap().is_artifact(),
        "face-down cost is empty"
    );
    engine.state.objects.get_mut(&support).unwrap().face_down = false;
    engine
        .state
        .continuous_effects
        .push(early_layer_type_effect(
            AffectedScope::Single(support),
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            1,
        ));
    assert!(
        engine.characteristics(source).unwrap().is_artifact(),
        "layer-two control overrides cached controller and battlefield vector"
    );
    engine.state.continuous_effects.pop();
    engine
        .state
        .objects
        .get_mut(&support)
        .unwrap()
        .copiable_values = None;
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", true);
    assert!(
        !engine.characteristics(source).unwrap().is_artifact(),
        "copied token retains mana symbols"
    );
    engine.state.objects.get_mut(&support).unwrap().zone = Zone::Graveyard;
    assert!(
        engine.characteristics(source).unwrap().is_artifact(),
        "only actual battlefield objects contribute"
    );
}

#[test]
fn devotion_live_condition_uses_the_same_symbols_as_early_layers() {
    let (mut engine, source) = devotion_engine();
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    let condition: GameCondition = serde_json::from_value(serde_json::json!({
        "Devotion": {"color": "Green", "min": 5, "max": 5}
    }))
    .unwrap();
    let context = ConditionContext {
        controller: 0,
        source_object_id: source,
        source_zone_change: 0,
        resolving_spell_id: None,
        stack_item: None,
        previous_effect_result: None,
    };
    assert!(!engine.condition_holds(&condition, context));
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    assert!(engine.condition_holds(&condition, context));
    let opponent = insert_fixture(&mut engine, 1, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, opponent, "{G}{G}{G}{G}{G}", false);
    assert!(
        engine.condition_holds(&condition, context),
        "other controllers excluded"
    );
    engine.state.objects.get_mut(&support).unwrap().face_down = true;
    assert!(!engine.condition_holds(&condition, context));
}

#[test]
fn devotion_remove_creature_preserves_artifact_and_legendary_in_both_timestamp_orders() {
    for (removal_time, addition_time) in [(1, 2), (2, 1)] {
        let source = r#"(
            id: "devotion_god", name: "Devotion God", face_id: "devotion_god",
            mana_cost: "{3}{G}", types: ["Enchantment", "Creature", "God"],
            supertypes: ["Legendary"], power: Some(6), toughness: Some(6),
            static_abilities: [(ability_id: "static_01", presentation: Fallback,
                definition: ConditionalSelfModifier(
                    condition: Devotion(color: Green, max: Some(4)),
                    remove_creature: true, add_types: (card_types: [Artifact])))],
        )"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[source], &[]).unwrap();
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            700_502,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        engine.registry = Box::leak(Box::new(registry));
        let source = insert_fixture(&mut engine, 0, "devotion_god", Zone::Battlefield);
        engine.state.command_index = removal_time;
        engine.emit_static_abilities_on_enter(source);
        engine
            .state
            .continuous_effects
            .push(early_layer_type_effect(
                AffectedScope::Single(source),
                ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Artifact],
                    ..Default::default()
                }),
                addition_time,
            ));
        let result = engine.characteristics(source).unwrap();
        assert!(!result.is_creature());
        assert!(!result.has_type("God"));
        assert!(result.is_artifact() && result.has_type("Enchantment") && result.is_legendary());
        engine
            .state
            .continuous_effects
            .push(early_layer_type_effect(
                AffectedScope::Single(source),
                ContinuousEffectKind::Layer6RemoveAllAbilities,
                3,
            ));
        let result = engine.characteristics(source).unwrap();
        assert!(
            !result.is_creature(),
            "later ability loss does not undo the layer-four effect"
        );
        assert!(result.is_artifact() && result.is_legendary());
    }
}

fn devotion_combat(engine: &mut GameEngine, attacker: ObjectId, blockers: Vec<ObjectId>) {
    let assignment = CombatAttackAssignment {
        attacker: engine.trigger_object_ref(attacker).unwrap(),
        defender: CombatDefenderTarget::Player(1),
        defending_player: 1,
    };
    let blocker_map = if blockers.is_empty() {
        HashMap::new()
    } else {
        HashMap::from([(attacker, blockers.clone())])
    };
    engine.state.combat = Some(CombatState {
        attacking: vec![attacker],
        attack_assignments: HashMap::from([(attacker, assignment)]),
        blockers: blocker_map.clone(),
        damage_assignments: HashMap::from([(
            attacker,
            blockers.iter().map(|id| (*id, 2)).collect(),
        )]),
        trample_player_damage: HashMap::from([(attacker, 2)]),
        damage_assignment_needed: false,
        attackers_declared: true,
        blockers_declared_by: vec![1],
        blockers_declared: true,
        assign_combat_damage_phase: true,
        first_strike_attackers: vec![attacker],
        first_strike_blockers: blocker_map,
        first_strike_damage_done: true,
    });
}

#[test]
fn devotion_instruction_boundary_removes_attacker_before_supporter_returns() {
    let (mut engine, god) = devotion_engine();
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    assert!(engine.characteristics(god).unwrap().is_creature());
    devotion_combat(&mut engine, god, vec![]);
    let mut events = vec![];
    engine.state.objects.get_mut(&support).unwrap().zone = Zone::Hand;
    assert!(!engine.characteristics(god).unwrap().is_creature());
    engine
        .drain_immediate_observer_actions(None, &mut events)
        .unwrap();
    assert!(
        engine.state.combat.as_ref().unwrap().attacking.is_empty(),
        "CR 506.4 acts before the next instruction"
    );
    engine.state.objects.get_mut(&support).unwrap().zone = Zone::Battlefield;
    engine
        .drain_immediate_observer_actions(None, &mut events)
        .unwrap();
    assert!(engine.characteristics(god).unwrap().is_creature());
    let combat = engine.state.combat.as_ref().unwrap();
    assert!(combat.attacking.is_empty() && combat.attack_assignments.is_empty());
    assert!(combat.first_strike_attackers.is_empty());
    assert!(combat.damage_assignments.is_empty() && combat.trample_player_damage.is_empty());
    assert!(!combat.damage_assignment_needed);
    let removals: Vec<_> = events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::RemovedFromCombat(removed)) => {
                Some(removed.object_ids.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        removals,
        vec![vec![god]],
        "recovered type never rejoins combat"
    );
}

#[test]
fn devotion_instruction_boundary_removes_blocker_but_preserves_blocked_status() {
    let (mut engine, god) = devotion_engine();
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    let attacker = insert_fixture(&mut engine, 1, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, attacker, "{G}", false);
    let face = &mut engine
        .state
        .objects
        .get_mut(&attacker)
        .unwrap()
        .copiable_values
        .as_mut()
        .unwrap()
        .face;
    face.types = vec!["Creature".into()];
    face.power = Some(4);
    face.toughness = Some(4);
    devotion_combat(&mut engine, attacker, vec![god]);
    engine.state.objects.get_mut(&support).unwrap().zone = Zone::Hand;
    let mut events = vec![];
    engine
        .drain_immediate_observer_actions(None, &mut events)
        .unwrap();
    let combat = engine.state.combat.as_ref().unwrap();
    assert_eq!(combat.attacking, vec![attacker]);
    assert_eq!(
        combat.blockers.get(&attacker),
        Some(&vec![]),
        "empty key retains blocked status"
    );
    assert!(combat
        .first_strike_blockers
        .get(&attacker)
        .unwrap()
        .is_empty());
    assert!(combat.damage_assignments.is_empty() && combat.trample_player_damage.is_empty());
    assert!(!combat.damage_assignment_needed);
    assert!(events.iter().any(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::RemovedFromCombat(removed)) if removed.object_ids == vec![god])));
}

#[test]
fn devotion_committed_departure_removes_attacker_before_nested_return() {
    let (mut engine, god) = devotion_engine();
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    devotion_combat(&mut engine, god, vec![]);
    let snapshot = engine.snapshot_zone_event();
    super::super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        support,
        Zone::Exile,
        None,
    )
    .unwrap();
    engine.fire_zone_triggers(snapshot, vec![], &mut Vec::new());
    assert!(
        engine.state.combat.as_ref().unwrap().attacking.is_empty(),
        "committed departure precedes a nested return or parked entry"
    );
}

#[test]
fn devotion_payment_removes_god_before_cost_triggers_or_payment_continuation() {
    let (mut engine, god) = devotion_engine();
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    devotion_combat(&mut engine, god, vec![]);
    let reference = |id| rv1::CostObjectRef {
        object_id: id,
        zone_change_generation: 0,
    };
    let cost = tricerules_card_model::primitives::ResolutionCost::SacrificePermanent {
        filter: TargetFilter {
            kind: TargetKind::AnyPermanent,
            ..Default::default()
        },
        source_only: false,
    };
    let plan = engine
        .plan_resolution_object_costs(0, reference(god), &cost, &[reference(support)])
        .unwrap();
    let receipt = engine.commit_cost_transaction(plan).unwrap();
    assert!(
        engine.state.combat.as_ref().unwrap().attacking.is_empty(),
        "cost commitment removes immediately, before triggers are fired"
    );
    assert!(receipt.move_events.iter().any(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::RemovedFromCombat(removed)) if removed.object_ids == vec![god])));
}

#[test]
fn devotion_control_reindex_removes_other_god_whose_supporter_changed_control() {
    let (mut engine, god) = devotion_engine();
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    devotion_combat(&mut engine, god, vec![]);
    engine
        .state
        .continuous_effects
        .push(early_layer_type_effect(
            AffectedScope::Single(support),
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            7,
        ));
    let mut events = vec![];
    engine.reindex_battlefield_control(&mut events);
    assert_eq!(engine.state.objects[&support].controller, 1);
    assert!(engine.state.combat.as_ref().unwrap().attacking.is_empty());
    assert!(events.iter().any(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::RemovedFromCombat(removed)) if removed.object_ids == vec![god])));
}

#[test]
fn devotion_regeneration_prunes_first_strike_and_damage_assignment_state() {
    let (mut engine, god) = devotion_engine();
    let support = insert_fixture(&mut engine, 0, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    devotion_combat(&mut engine, god, vec![]);
    engine
        .state
        .objects
        .get_mut(&god)
        .unwrap()
        .regeneration_shields = 1;
    let mut events = vec![];
    assert!(super::super::super::resolution::consume_regen_shield(&mut engine, god, &mut events).0);
    let combat = engine.state.combat.as_ref().unwrap();
    assert!(combat.first_strike_attackers.is_empty() && combat.attack_assignments.is_empty());
    assert!(combat.damage_assignments.is_empty() && combat.trample_player_damage.is_empty());
}

#[test]
fn devotion_transform_flip_room_and_copied_room_use_costs_rather_than_mana_value() {
    let mut engine = GameEngine::new_with_default_decks(
        tricerules_cards::registry::global(),
        700_503,
        &[0, 1],
        20,
    )
    .unwrap();
    let transform = insert_fixture(
        &mut engine,
        0,
        "village_ironsmith_ironfang",
        Zone::Battlefield,
    );
    let flip = insert_fixture(
        &mut engine,
        0,
        "akki_lavarunner_tok-tok,_volcano_born",
        Zone::Battlefield,
    );
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Red, None),
        2
    );
    engine
        .state
        .objects
        .get_mut(&transform)
        .unwrap()
        .face_up_index = 1;
    engine.state.objects.get_mut(&flip).unwrap().face_up_index = 1;
    assert_eq!(engine.characteristics(transform).unwrap().mana_value, 2);
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Red, None),
        1,
        "transform front mana value is retained but its red symbol is not; flip retains its cost"
    );
    let room = insert_fixture(
        &mut engine,
        0,
        "derelict_attic_widows_walk",
        Zone::Battlefield,
    );
    engine.state.room_states.insert(room, RoomState::default());
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Black, None),
        0
    );
    engine.transition_room_door(room, 0).unwrap();
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Black, None),
        1
    );
    engine.transition_room_door(room, 1).unwrap();
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Black, None),
        2
    );
    let copy = insert_fixture(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
    engine.state.objects.get_mut(&copy).unwrap().copiable_values =
        Some(engine.copiable_values_for(room).unwrap());
    engine.state.room_states.insert(copy, RoomState::default());
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Black, None),
        2,
        "copied Room starts locked"
    );
    engine.transition_room_door(copy, 1).unwrap();
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Black, None),
        3
    );
}

#[test]
fn devotion_mdfc_uses_active_cost_and_queries_the_current_source_controller() {
    let card = r#"(id: "devotion_front_devotion_back", name: "Devotion Front // Devotion Back", layout: ModalDfc,
        faces: [(name: "Devotion Front", face_id: "devotion_front", mana_cost: "{G}{G}{G}", types: ["Creature"], power: Some(3), toughness: Some(3)),
                (name: "Devotion Back", face_id: "devotion_back", mana_cost: "{R}{R}", types: ["Creature"], power: Some(2), toughness: Some(2))])"#;
    let registry = CardRegistry::from_chunks_and_tokens(&[card], &[]).unwrap();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        700_504,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    engine.registry = Box::leak(Box::new(registry));
    let oid = insert_fixture(
        &mut engine,
        0,
        "devotion_front_devotion_back",
        Zone::Battlefield,
    );
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Green, None),
        3
    );
    engine.state.objects.get_mut(&oid).unwrap().face_up_index = 1;
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Green, None),
        0
    );
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Red, None),
        2
    );
    engine
        .state
        .continuous_effects
        .push(early_layer_type_effect(
            AffectedScope::Single(oid),
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            2,
        ));
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 0, Color::Red, None),
        0
    );
    assert_eq!(
        devotion_value(&engine.state, engine.registry, 1, Color::Red, None),
        2
    );
}

#[test]
fn devotion_independent_creature_addition_respects_timestamps_and_earlier_text_removal() {
    for (removal_time, addition_time, expected_creature) in [(1, 2, true), (2, 1, false)] {
        let (mut engine, god) = devotion_engine();
        for effect in &mut engine.state.continuous_effects {
            effect.timestamp = removal_time;
        }
        engine
            .state
            .continuous_effects
            .push(early_layer_type_effect(
                AffectedScope::Single(god),
                ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    creature_types: vec!["Plant".into()],
                    ..Default::default()
                }),
                addition_time,
            ));
        let result = engine.characteristics(god).unwrap();
        assert_eq!(result.is_creature(), expected_creature);
        assert_eq!(result.has_type("Plant"), expected_creature);
        assert!(result.is_artifact() && result.is_legendary());
    }
    let (mut engine, god) = devotion_engine();
    engine
        .state
        .continuous_effects
        .push(early_layer_type_effect(
            AffectedScope::Single(god),
            ContinuousEffectKind::Layer4SetTypeLine(tricerules_card_model::TypeLineReplacement {
                card_types: vec![PermanentTypeFilter::Land],
                land_types: vec![BasicLandType::Forest],
                creature_types: vec![],
            }),
            2,
        ));
    let result = engine.characteristics(god).unwrap();
    assert!(result.has_type("Land") && result.has_type("Forest"));
    assert!(!result.is_artifact(), "earlier-layer printed-text removal suppresses both components of the intrinsic static ability");
}

#[test]
fn devotion_projected_entry_excludes_own_pip_and_uses_destination_controller() {
    let (mut engine, god) = devotion_engine();
    let support = insert_fixture(&mut engine, 1, "devotion_support", Zone::Battlefield);
    copy_cost(&mut engine, support, "{G}{G}{G}{G}", false);
    let event = BattlefieldEntryEvent {
        object_id: god,
        deciding_player: 1,
        destination_controller: 1,
        entry_reveal_receipts: vec![],
        mana_colors_spent_to_cast: Default::default(),
        prepared: false,
        battle_protector: None,
        face_index: 0,
        unlock_room_door: None,
        chosen_x: 0,
        cast_by: None,
        cast_cost_receipts: vec![],
        player_life_snapshot: engine.player_life_snapshot(),
        tapped: false,
        set_types: None,
        chosen_basic_land_type: None,
        chosen_opponents: vec![],
        entry_counters: BTreeMap::new(),
        entry_modifiers: vec![],
        attached_to: None,
        pending_copy_candidate: None,
        pending_aura_recipient: None,
        accepted_aura_recipient: None,
        applied_effects: vec![],
    };
    let face = engine
        .registry
        .get("devotion_source")
        .unwrap()
        .primary_face();
    let statics: Vec<_> = face
        .static_abilities
        .iter()
        .map(|ability| {
            (
                engine.ability_definition(god, 0, vec![ability.ability_id.clone()]),
                &ability.definition,
            )
        })
        .collect();
    let (projected, _) = entry_characteristics_through_layer_5(
        &engine.state,
        engine.registry,
        &event,
        face,
        &statics,
        &[],
    )
    .unwrap();
    assert_eq!(projected.controller, 1);
    assert!(
        !projected.is_creature(),
        "four destination-controller pips exclude the projected entrant"
    );
    engine.state.players[0].battlefield.retain(|id| *id != god);
    engine.state.players[1].battlefield.push(god);
    let object = engine.state.objects.get_mut(&god).unwrap();
    object.controller = 1;
    object.base_controller = 1;
    let (projected, _) = entry_characteristics_through_layer_5(
        &engine.state,
        engine.registry,
        &event,
        face,
        &statics,
        &[],
    )
    .unwrap();
    assert!(
        !projected.is_creature(),
        "even an entry whose provisional state contains a battlefield object excludes its pip"
    );
    engine.refresh_source_static_abilities(god);
    assert!(
        engine.characteristics(god).unwrap().is_creature(),
        "the committed own pip raises devotion to five"
    );
}
