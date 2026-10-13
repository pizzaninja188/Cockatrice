//! Actual-card coverage for Chimil's controller-scoped counter protection and Discover.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{CardRegistry, ContinuousEffectKind, ControllerReference, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, EngineDeck, TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::TargetRefKind;

const CHIMIL: &str = "chimil,_the_inner_sun";

const DISCOVER_MODAL_DFC: &str = r#"
(
  id: "test_discover_modal_dfc",
  name: "Test Discover // High Spell",
  layout: ModalDfc,
  faces: [
    (name: "Test Discover", face_id: "test_discover_front", mana_cost: "{2}", types: ["Sorcery"], spell_effect: [GainLife(amount: 1)]),
    (name: "High Spell", face_id: "test_discover_back", mana_cost: "{6}", types: ["Sorcery"], spell_effect: [GainLife(amount: 2)]),
  ],
)
"#;

const DISCOVER_NO_TARGET_SPELL: &str = r#"
(
  id: "test_discover_no_target_spell",
  name: "Test Discover No Target",
  face_id: "test_discover_no_target_spell",
  mana_cost: "{2}",
  types: ["Instant"],
  spell_effect: [DamageTarget(amount: 1, target: (kind: Creature, controller: Opponent))],
  targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature an opponent controls", effect_indices: [0])])),
)
"#;

const DISCOVER_WITH_SURVIVOR_TAIL: &str = r#"
(
    id: "test_discover_with_survivor_tail",
    name: "Test Discover with Survivor Tail",
    face_id: "test_discover_with_survivor_tail",
    mana_cost: "{6}",
    types: ["Artifact"],
    triggered_abilities: [(
        ability_id: "triggered_01",
        presentation: Fallback,
        trigger: AtBeginningOfEndStep(player: Controller),
        effect: [Discover(value: 5), LoseLife(amount: Fixed(1), who: EachOpponent)],
    )],
)
"#;

fn set_library_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn discover_offer_with_cohort(seed: u64, top_card_ids: &[&str]) -> (GameEngine, u32, Vec<u32>) {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let chimil = inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    let cohort = set_library_top(&mut engine, 0, top_card_ids);
    for _ in 0..32 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        pass_priority_round(&mut engine);
        if engine.state.turn_step == TurnStep::Cleanup {
            break;
        }
    }
    assert!(engine.state.pending_resolution.is_some());
    (engine, chimil, cohort)
}

fn discover_cast_announcement(
    engine: &mut GameEngine,
    object_id: u32,
    target_player_id: i32,
) -> rv1::SpellCastAnnouncement {
    let generation = engine.state.zone_change_generation[&object_id];
    let action = engine.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == object_id)
        .cloned()
        .expect("Discover publishes the qualifying spell");
    rv1::SpellCastAnnouncement {
        targets: target_player(target_player_id),
        face_index: action.face_index,
        source: Some(exile_cast_source(object_id, generation)),
        cast_method: action.cast_method,
        casting_permission_id: action.casting_permission_id,
        ..Default::default()
    }
}

fn discover_offer_cohort_snapshot(engine: &GameEngine) -> Vec<(u32, u64)> {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Discover cast offer is pending");
    let tricerules_core::state::ResolutionContinuation::SpecialCast {
        exiled,
        completion: tricerules_core::state::SpecialCastCompletion::Discover { found, cohort, .. },
        ..
    } = &pending.continuation
    else {
        panic!("Discover owns the outstanding special-cast offer");
    };
    assert_eq!(
        Some(exiled.object_id),
        found.map(|reference| reference.object_id)
    );
    cohort
        .iter()
        .map(|reference| (reference.object_id, reference.zone_change_generation))
        .collect()
}

fn discover_zone_view(batch: &rv1::RuledEventBatch) -> &rv1::ZoneViewSync {
    batch
        .events
        .iter()
        .rev()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("Discover cleanup emits the final zone view")
}

fn discover_player_view(view: &rv1::ZoneViewSync, player_id: i32) -> &rv1::RuledPerPlayerView {
    view.per_player
        .iter()
        .find(|player| player.player_id == player_id)
        .expect("zone view includes each player")
}

fn commander_discover_engine(seed: u64, commander: &str) -> (GameEngine, u32, u32) {
    let mainboard_land = if commander == "kokusho,_the_evening_star" {
        "mountain"
    } else {
        "forest"
    };
    let decks = Some(vec![
        EngineDeck {
            mainboard: vec![mainboard_land.to_owned(); 8],
            commanders: vec![commander.to_owned()],
        },
        EngineDeck {
            mainboard: vec!["island".to_owned(); 8],
            commanders: vec!["kami_of_the_crescent_moon".to_owned()],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new Commander Discover game");
    advance_to_main1_from_game_start(&mut engine);
    let commander = engine.state.players[0].command_zone[0];
    engine.state.players[0]
        .command_zone
        .retain(|object_id| *object_id != commander);
    engine.state.players[0].library.push_front(commander);
    let object = engine.state.objects.get_mut(&commander).unwrap();
    object.zone = Zone::Library;
    object.controller = 0;
    object.base_controller = 0;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_default() += 1;
    let chimil = inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    (engine, chimil, commander)
}

fn discover_commander_observer_engine(
    seed: u64,
    return_count: usize,
) -> (GameEngine, u32, u32, u32, Vec<u32>) {
    let mut banishing_light_deck = vec!["plains".to_owned(); 8];
    for _ in 0..return_count {
        banishing_light_deck.push("banishing_light".to_owned());
    }
    let commander = "atraxa,_praetors_voice".to_owned();
    let decks = Some(vec![
        EngineDeck {
            mainboard: vec!["forest".to_owned(); 8],
            commanders: vec![commander.clone()],
        },
        EngineDeck {
            mainboard: banishing_light_deck,
            commanders: vec![commander.clone()],
        },
        EngineDeck {
            mainboard: vec!["plains".to_owned(); 8],
            commanders: vec![commander],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        40,
        decks,
        true,
    )
    .expect("new three-player Commander Discover game");
    advance_to_main1_from_game_start(&mut engine);

    let commander = engine.state.players[0].command_zone[0];
    engine.state.players[0]
        .command_zone
        .retain(|object_id| *object_id != commander);
    engine.state.players[0].library.push_front(commander);
    let object = engine.state.objects.get_mut(&commander).unwrap();
    object.zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_default() += 1;

    let steels = (0..return_count)
        .map(|_| inject_permanent_on_battlefield(&mut engine, 0, "sculpting_steel"))
        .collect::<Vec<_>>();
    advance_to_active_main1(&mut engine, 1);
    for steel in &steels {
        ensure_card_in_hand(&mut engine, 1, "banishing_light");
        give_mana(
            &mut engine,
            1,
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 1, "banishing_light");
        engine
            .apply_scenario_command(1, &cast_spell(slot, Vec::new()))
            .expect("P1 casts Banishing Light");
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.pending_triggers.len(), 1);
        engine
            .apply_scenario_command(
                1,
                &RuledCommand {
                    cmd: Some(Cmd::ChooseTriggerTarget(rv1::ChooseTriggerTarget {
                        targets: target_object(*steel),
                        ..Default::default()
                    })),
                },
            )
            .expect("P1 chooses the Sculpting Steel target");
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.objects[steel].zone, Zone::Exile);
    }
    assert_eq!(engine.state.active_event_observers.len(), return_count);

    advance_to_active_main1(&mut engine, 0);
    engine.state.players[0]
        .hand
        .retain(|object_id| *object_id != commander);
    engine.state.players[0]
        .library
        .retain(|object_id| *object_id != commander);
    engine.state.players[0].library.push_front(commander);
    let object = engine.state.objects.get_mut(&commander).unwrap();
    object.zone = Zone::Library;
    object.controller = 0;
    object.base_controller = 0;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_default() += 1;
    let forest = set_library_top(&mut engine, 0, &["forest"])[0];
    let chimil = inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    for _ in 0..96 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        pass_priority_round(&mut engine);
        assert_ne!(engine.state.turn_step, TurnStep::Cleanup);
    }
    assert!(engine.state.pending_resolution.is_some());
    let cohort_before = discover_offer_cohort_snapshot(&engine);
    assert_eq!(
        cohort_before
            .iter()
            .map(|(object_id, _)| *object_id)
            .collect::<Vec<_>>(),
        [forest, commander],
        "Discover retains the exact land prefix and commander hit"
    );
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
        )
        .expect("decline casting the discovered commander");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Discover pauses on the commander replacement")
            .deciding_player,
        0
    );
    (engine, chimil, commander, forest, steels)
}

fn discover_departed_owner_observer_engine(
    seed: u64,
    stop_on_commander_choice: bool,
) -> (GameEngine, u32, u32, u32, u32, u32) {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/plains.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/atraxa_praetors_voice.ron"),
                include_str!("../../../tricerules-cards/data/banishing_light.ron"),
                include_str!("../../../tricerules-cards/data/sculpting_steel.ron"),
                include_str!("../../../tricerules-cards/data/sol_ring.ron"),
                DISCOVER_WITH_SURVIVOR_TAIL,
            ],
            &[],
        )
        .expect("test-only Discover continuation registry"),
    ));
    let commander = "atraxa,_praetors_voice".to_owned();
    let mut p1_mainboard = vec!["plains".to_owned(); 8];
    p1_mainboard.push("banishing_light".to_owned());
    let mut p3_mainboard = vec!["island".to_owned(); 8];
    p3_mainboard.push("sol_ring".to_owned());
    let decks = Some(vec![
        EngineDeck {
            mainboard: vec!["forest".to_owned(); 8],
            commanders: vec![commander.clone()],
        },
        EngineDeck {
            mainboard: p1_mainboard,
            commanders: vec![commander.clone()],
        },
        EngineDeck {
            mainboard: vec!["plains".to_owned(); 8],
            commanders: vec![commander.clone()],
        },
        EngineDeck {
            mainboard: p3_mainboard,
            commanders: vec![commander],
        },
    ]);
    let mut engine =
        GameEngine::new_with_commander_decks(registry, seed, &[0, 1, 2, 3], 40, decks, true)
            .expect("new four-player Discover departure game");
    advance_to_main1_from_game_start(&mut engine);

    let p0_commander = engine.state.players[0].command_zone[0];
    engine.state.players[0]
        .command_zone
        .retain(|object_id| *object_id != p0_commander);
    engine.state.players[0].library.push_front(p0_commander);
    engine.state.objects.get_mut(&p0_commander).unwrap().zone = Zone::Library;
    let steel = inject_permanent_on_battlefield(&mut engine, 2, "sculpting_steel");
    let surviving_artifact = inject_permanent_on_battlefield(&mut engine, 3, "sol_ring");

    advance_to_active_main1(&mut engine, 1);
    ensure_card_in_hand(&mut engine, 1, "banishing_light");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let banishing_light_slot = hand_index_for_card(&engine, 1, "banishing_light");
    engine
        .apply_scenario_command(1, &cast_spell(banishing_light_slot, Vec::new()))
        .expect("P1 casts Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(rv1::ChooseTriggerTarget {
                    targets: target_object(steel),
                    ..Default::default()
                })),
            },
        )
        .expect("P1 targets P2's Sculpting Steel");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&steel].zone, Zone::Exile);
    assert_eq!(engine.state.active_event_observers.len(), 1);

    advance_to_active_main1(&mut engine, 0);
    engine.state.players[0]
        .hand
        .retain(|object_id| *object_id != p0_commander);
    engine.state.players[0]
        .library
        .retain(|object_id| *object_id != p0_commander);
    engine.state.players[0].library.push_front(p0_commander);
    engine.state.objects.get_mut(&p0_commander).unwrap().zone = Zone::Library;
    let forest = set_library_top(&mut engine, 0, &["forest"])[0];
    let source =
        inject_permanent_on_battlefield(&mut engine, 0, "test_discover_with_survivor_tail");
    for _ in 0..96 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        pass_priority_round(&mut engine);
        assert_ne!(engine.state.turn_step, TurnStep::Cleanup);
    }
    assert!(engine.state.pending_resolution.is_some());
    let cohort = discover_offer_cohort_snapshot(&engine);
    assert_eq!(
        cohort
            .iter()
            .map(|(object_id, _)| *object_id)
            .collect::<Vec<_>>(),
        [forest, p0_commander],
        "the paused Discover retains the exact forest and commander cohort"
    );
    if stop_on_commander_choice {
        engine
            .apply_scenario_command(
                0,
                &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
            )
            .expect("decline casting the discovered commander");
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(
                tricerules_core::state::ResolutionContinuation::SpecialCastCommanderCleanup { .. }
            )
        ));
    }

    (
        engine,
        source,
        p0_commander,
        forest,
        steel,
        surviving_artifact,
    )
}

fn advance_to_active_main1(engine: &mut GameEngine, player_id: i32) {
    for _ in 0..96 {
        if engine.state.active_player_id() == player_id && engine.state.turn_step == TurnStep::Main1
        {
            return;
        }
        if let Some(discarding_player) = engine.state.cleanup_discard_player {
            engine
                .apply_scenario_command(discarding_player, &discard_cleanup(0))
                .expect("discard to hand size while advancing the scenario turn");
            continue;
        }
        pass_priority_round(engine);
    }
    panic!("P{player_id} did not reach main phase 1");
}

fn discover_commander_branch_offer(
    batch: &rv1::RuledEventBatch,
) -> Option<&rv1::ResolutionChoiceRequired> {
    batch.events.iter().find_map(|event| match &event.ev {
        Some(Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == rv1::ChoiceKind::ResolutionBranch as i32 =>
        {
            Some(choice)
        }
        _ => None,
    })
}

fn resolve_discover_observer_returns(
    engine: &mut GameEngine,
    chimil: u32,
) -> Vec<rv1::RuledEventBatch> {
    let mut observer_batches = Vec::new();
    for _ in 0..8 {
        observer_batches.extend(
            answer_simultaneous_entry_order_in_engine_order(engine)
                .into_iter()
                .map(|(_, _, batch)| batch),
        );
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the return batch parks for its entry choices");
        if pending.presentation.choice_kind == ChoiceKind::ResolutionBranch {
            return observer_batches;
        }
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
        assert!(pending.presentation.candidates.contains(&chimil));
        observer_batches.push(
            engine
                .apply_scenario_command(
                    pending.deciding_player,
                    &submit_resolution_choice(vec![chimil]),
                )
                .expect("choose Chimil as the Sculpting Steel copy source"),
        );
    }
    panic!("observer returns did not restore Discover's commander choice");
}

fn submit_discover_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn prepared_arcane_denial(seed: u64) -> (GameEngine, u32, u32) {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let chimil = inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    inject_card_into_hand(&mut engine, 0, "opt");
    inject_card_into_hand(&mut engine, 1, "arcane_denial");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
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

    let opt = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "opt")
        .expect("Opt in hand");
    let opt_slot = hand_index_for_card(&engine, 0, "opt");
    engine
        .apply_scenario_command(0, &cast_spell(opt_slot, Vec::new()))
        .expect("cast Opt under Chimil");
    assert_eq!(engine.state.stack.last().unwrap().id, opt);
    engine
        .apply_scenario_command(0, &pass())
        .expect("Opt's controller passes");
    let denial_slot = hand_index_for_card(&engine, 1, "arcane_denial");
    engine
        .apply_scenario_command(1, &cast_spell(denial_slot, stack_target(opt)))
        .expect("counterspell can still target Chimil-protected spell");
    (engine, chimil, opt)
}

fn change_chimil_controller(engine: &mut GameEngine, chimil: u32, player: i32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(chimil),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(player),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + engine.state.continuous_effects.len() as u64 + 1,
    });
    engine.initial_response_batch();
}

fn remove_chimil_abilities(engine: &mut GameEngine, chimil: u32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(chimil),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index + engine.state.continuous_effects.len() as u64 + 1,
    });
    engine.initial_response_batch();
}

#[test]
fn chimil_keeps_controller_spell_on_stack_and_arcane_denial_tail_resolves() {
    let (mut engine, _, opt) = prepared_arcane_denial(1_197_001);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, opt);
    assert_eq!(engine.state.objects[&opt].zone, Zone::Stack);
    assert_eq!(engine.state.active_event_observers.len(), 2);
}

#[test]
fn chimil_protection_tracks_source_control_and_ability_availability_at_resolution() {
    for remove_abilities in [false, true] {
        let (mut engine, chimil, opt) = prepared_arcane_denial(if remove_abilities {
            1_197_003
        } else {
            1_197_002
        });
        if remove_abilities {
            remove_chimil_abilities(&mut engine, chimil);
        } else {
            change_chimil_controller(&mut engine, chimil, 1);
        }
        pass_priority_round(&mut engine);
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.objects[&opt].zone, Zone::Graveyard);
        assert_eq!(engine.state.active_event_observers.len(), 2);
    }
}

#[test]
fn chimil_spell_is_counterable_before_it_reaches_the_battlefield() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        1_197_004,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, CHIMIL);
    inject_card_into_hand(&mut engine, 1, "counterspell");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 6,
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
    let chimil_slot = hand_index_for_card(&engine, 0, CHIMIL);
    engine
        .apply_scenario_command(0, &cast_spell(chimil_slot, Vec::new()))
        .expect("cast Chimil");
    let chimil = engine.state.stack.last().unwrap().id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("Chimil's caster passes");
    let counter_slot = hand_index_for_card(&engine, 1, "counterspell");
    engine
        .apply_scenario_command(1, &cast_spell(counter_slot, stack_target(chimil)))
        .expect("Chimil on the stack is a legal counter target");
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&chimil].zone, Zone::Graveyard);
}

#[test]
fn chimil_protection_also_stops_its_controller_from_countering_their_spell() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        1_197_010,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    inject_card_into_hand(&mut engine, 0, "opt");
    inject_card_into_hand(&mut engine, 0, "counterspell");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );

    let opt_slot = hand_index_for_card(&engine, 0, "opt");
    engine
        .apply_scenario_command(0, &cast_spell(opt_slot, Vec::new()))
        .expect("cast Opt under Chimil");
    let opt = engine.state.stack.last().unwrap().id;
    let counterspell = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "counterspell")
        .expect("Counterspell in hand");
    let counterspell_slot = hand_index_for_card(&engine, 0, "counterspell");
    engine
        .apply_scenario_command(0, &cast_spell(counterspell_slot, stack_target(opt)))
        .expect("cast Counterspell at the protected spell");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, opt);
    assert_eq!(engine.state.objects[&opt].zone, Zone::Stack);
    assert_eq!(engine.state.objects[&counterspell].zone, Zone::Graveyard);
}

fn discover_offer(seed: u64) -> (GameEngine, u32, u32, u32, u32) {
    // Advance through the combat and second main phase; the end-step trigger resolves into a cast
    // offer, leaving the exact prefix exiled while the controller decides.
    let (engine, chimil, cohort) =
        discover_offer_with_cohort(seed, &["forest", "portal_to_phyrexia", "lightning_bolt"]);
    let [land, over_limit, found] = cohort.try_into().expect("three-card Discover prefix");
    (engine, chimil, land, over_limit, found)
}

fn discover_three_player_tail_offer(seed: u64) -> (GameEngine, u32, Vec<u32>) {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/lightning_bolt.ron"),
                DISCOVER_WITH_SURVIVOR_TAIL,
            ],
            &[],
        )
        .expect("test-only Discover transaction registry"),
    ));
    let deck = vec!["island".to_owned(); 8];
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1, 2],
        40,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("new three-player Discover transaction game");
    advance_to_main1_from_game_start(&mut engine);
    let source =
        inject_permanent_on_battlefield(&mut engine, 0, "test_discover_with_survivor_tail");
    let cohort = set_library_top(&mut engine, 0, &["forest", "lightning_bolt"]);
    for _ in 0..96 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        pass_priority_round(&mut engine);
        assert_ne!(engine.state.turn_step, TurnStep::Cleanup);
    }
    assert!(engine.state.pending_resolution.is_some());
    (engine, source, cohort)
}

#[test]
fn discover_exhausts_the_library_without_a_hit_and_handles_an_empty_library() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        1_197_007,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    let [land, over_limit] = set_library_top(&mut engine, 0, &["forest", "portal_to_phyrexia"])
        .try_into()
        .expect("two-card no-hit prefix");
    for _ in 0..32 {
        if engine.state.turn_step == TurnStep::Cleanup {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.turn_step, TurnStep::Cleanup);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&land].zone, Zone::Library);
    assert_eq!(engine.state.objects[&over_limit].zone, Zone::Library);
    assert!(engine
        .state
        .objects
        .values()
        .all(|object| object.zone != Zone::Exile));

    let mut empty_engine = GameEngine::new(
        tricerules_cards::registry::global(),
        1_197_009,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("island", &[])]),
        true,
    )
    .expect("new game with an empty-library fixture");
    advance_to_main1_from_game_start(&mut empty_engine);
    inject_permanent_on_battlefield(&mut empty_engine, 0, CHIMIL);
    empty_engine.state.players[0].library.clear();
    for _ in 0..32 {
        if empty_engine.state.turn_step == TurnStep::Cleanup {
            break;
        }
        pass_priority_round(&mut empty_engine);
    }
    assert_eq!(empty_engine.state.turn_step, TurnStep::Cleanup);
    assert!(empty_engine.state.pending_resolution.is_none());
    assert!(empty_engine.state.players[0].library.is_empty());
}

#[test]
fn discover_caps_every_offered_and_accepted_face_by_stack_mana_value() {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/chimil,_the_inner_sun.ron"),
                DISCOVER_MODAL_DFC,
            ],
            &[],
        )
        .expect("test-only Discover registry"),
    ));
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        registry,
        1_197_008,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    let [found] = set_library_top(&mut engine, 0, &["test_discover_modal_dfc"])
        .try_into()
        .expect("single Discover hit");
    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        pass_priority_round(&mut engine);
        if engine.state.turn_step == TurnStep::Cleanup {
            break;
        }
    }
    assert_eq!(engine.state.objects[&found].zone, Zone::Exile);
    let actions = engine.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .filter(|action| action.object_id == found)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].face_index, 0);
    assert_eq!(actions[0].cost, "");

    let generation = engine.state.zone_change_generation[&found];
    let illegal_back_face = CastSpell {
        source: Some(exile_cast_source(found, generation)),
        face_index: 1,
        cast_method: actions[0].cast_method,
        casting_permission_id: actions[0].casting_permission_id,
        ..Default::default()
    };
    let rejected = engine.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                resolution_prompt_id: 0,
                decision: ResolutionChoiceDecision::CastSpell as i32,
                cast_spell: Some(illegal_back_face),
                ..Default::default()
            })),
        },
    );
    assert!(
        rejected.is_err(),
        "Discover cannot cast a face above its limit"
    );
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&found].zone, Zone::Exile);

    let accepted_front_face = CastSpell {
        source: Some(exile_cast_source(found, generation)),
        face_index: actions[0].face_index,
        cast_method: actions[0].cast_method,
        casting_permission_id: actions[0].casting_permission_id,
        ..Default::default()
    };
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: ResolutionChoiceDecision::CastSpell as i32,
                    cast_spell: Some(accepted_front_face),
                    ..Default::default()
                })),
            },
        )
        .expect("cast the allowed face");
    assert_eq!(engine.state.objects[&found].zone, Zone::Stack);
    assert_eq!(engine.state.stack.last().unwrap().face_index, 0);
}

#[test]
fn discover_returns_the_hit_to_hand_when_no_spell_target_is_legal() {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/chimil,_the_inner_sun.ron"),
                DISCOVER_NO_TARGET_SPELL,
            ],
            &[],
        )
        .expect("test-only Discover no-target registry"),
    ));
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        registry,
        1_197_011,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    let [found] = set_library_top(&mut engine, 0, &["test_discover_no_target_spell"])
        .try_into()
        .expect("single target-locked Discover hit");
    for _ in 0..32 {
        if engine.state.pending_resolution.is_some() || engine.state.turn_step == TurnStep::Cleanup
        {
            break;
        }
        pass_priority_round(&mut engine);
    }

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.turn_step, TurnStep::Cleanup);
    assert_eq!(engine.state.objects[&found].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&found));
}

#[test]
fn discover_target_preview_honors_the_exiled_spell_faces_protection() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        1_197_012,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, CHIMIL);
    inject_permanent_on_battlefield(&mut engine, 1, "black_knight");
    let [swords] = set_library_top(&mut engine, 0, &["swords_to_plowshares"])
        .try_into()
        .expect("single Discover hit");
    for _ in 0..32 {
        if engine.state.pending_resolution.is_some() || engine.state.turn_step == TurnStep::Cleanup
        {
            break;
        }
        pass_priority_round(&mut engine);
    }

    assert!(
        engine.state.pending_resolution.is_none(),
        "the protected Black Knight is Swords' only creature target, so Discover must not offer the cast"
    );
    assert_eq!(engine.state.objects[&swords].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&swords));
}

#[test]
fn discover_spell_cast_cancellation_restores_the_exact_offer_and_resumes_cleanup() {
    let (mut engine, _chimil, land, over_limit, found) = discover_offer(1_197_015);
    let cohort_before = discover_offer_cohort_snapshot(&engine);
    let found_generation = engine.state.zone_change_generation[&found];
    let parent_prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Discover choice")
        .resolution_prompt_id
        .get();
    assert_ne!(parent_prompt_id, 0);
    let announcement = discover_cast_announcement(&mut engine, found, 1);
    let begin = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
            spell_cast_announcement: Some(announcement),
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(0, &begin)
        .expect("begin the Discover cast transaction");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Discover cast is awaiting payment")
        .transaction_id;
    assert_eq!(engine.state.objects[&found].zone, Zone::Stack);
    assert!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .expect("Discover cast is awaiting payment")
            .resolution_time_offer
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("parked parent remains suspended")
            .resolution_prompt_id
            .get(),
        0,
        "the accepted parent offer is retired while child payment is active"
    );
    let reconnect = engine.initial_response_batch();
    assert!(
        find_resolution_choice(&reconnect).is_none(),
        "reconnect during child payment does not republish the suspended parent offer"
    );

    let mut stale_parent_answer =
        submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline);
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_parent_answer.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = parent_prompt_id;
    let state_before_stale_answer = format!("{:?}", engine.state);
    let command_index_before_stale_answer = engine.state.command_index;
    assert!(engine.apply_command(0, &stale_parent_answer).is_err());
    assert_eq!(format!("{:?}", engine.state), state_before_stale_answer);
    assert_eq!(
        engine.state.command_index,
        command_index_before_stale_answer
    );

    let cancel_batch = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(rv1::CancelSpellCast {
                    transaction_id,
                })),
            },
        )
        .expect("cancel restores the Discover offer");

    let restored_offer = find_resolution_choice(&cancel_batch)
        .expect("cancel republishes the restored parent offer");
    assert_ne!(restored_offer.resolution_prompt_id, parent_prompt_id);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Discover parent restored")
            .resolution_prompt_id
            .get(),
        restored_offer.resolution_prompt_id
    );

    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_trigger_order.is_none());
    assert_eq!(discover_offer_cohort_snapshot(&engine), cohort_before);
    assert_eq!(
        engine.state.zone_change_generation[&found],
        found_generation
    );
    assert_eq!(engine.state.objects[&found].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&land].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&over_limit].zone, Zone::Exile);

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: restored_offer.resolution_prompt_id,
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("decline after cancellation and finish Discover");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&found].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&land].zone, Zone::Library);
    assert_eq!(engine.state.objects[&over_limit].zone, Zone::Library);
}

#[test]
fn discover_live_reserved_cast_survives_an_unrelated_concession_and_commits() {
    let (mut engine, _source, cohort) = discover_three_player_tail_offer(1_197_021);
    let found = cohort[1];
    let prefix = cohort[0];
    let cohort_before = discover_offer_cohort_snapshot(&engine);
    let found_generation = engine.state.zone_change_generation[&found];
    let announcement = discover_cast_announcement(&mut engine, found, 1);
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(announcement),
                    ..Default::default()
                })),
            },
        )
        .expect("begin the Discover cast transaction");
    let transaction = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Discover cast awaits payment")
        .clone();
    assert!(transaction.resolution_time_offer);
    assert_eq!(transaction.caster, 0);
    assert_eq!(transaction.reserved_object_id, found);
    assert_eq!(transaction.original_zone, Zone::Exile);
    assert_eq!(transaction.original_holder, 0);
    assert_eq!(
        transaction.original_zone_change_generation,
        found_generation
    );
    assert_eq!(engine.state.objects[&found].zone, Zone::Stack);
    assert_eq!(engine.state.objects[&prefix].zone, Zone::Exile);

    let survivor_life = engine.state.players[1].life;
    engine
        .apply_scenario_command(2, &concede())
        .expect("an unrelated player concedes while Discover cast payment is open");

    assert!(engine.state.players[2].has_lost);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .map(|cast| cast.transaction_id),
        Some(transaction.transaction_id),
        "the exact reserved cast transaction remains active"
    );
    assert_eq!(engine.state.objects[&found].zone, Zone::Stack);
    assert_eq!(engine.state.objects[&prefix].zone, Zone::Exile);
    assert_eq!(discover_offer_cohort_snapshot(&engine), cohort_before);
    assert!(engine.state.pending_resolution.is_some());

    let commit = rv1::CommitSpellCast {
        transaction_id: transaction.transaction_id,
        ..Default::default()
    };
    let preview = engine.preview_payment_scenario(
        0,
        &rv1::PreviewPayment {
            transaction_id: transaction.transaction_id,
            revision: engine.state.command_index,
            commit_spell_cast: Some(commit),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id: transaction.transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("the preserved Discover transaction commits");

    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&found].zone, Zone::Stack);
    assert_eq!(engine.state.objects[&prefix].zone, Zone::Library);
    assert_eq!(engine.state.players[1].life, survivor_life - 1);
}

#[test]
fn discover_live_reserved_cast_cancellation_after_concession_restores_the_exact_offer() {
    let (mut engine, _source, cohort) = discover_three_player_tail_offer(1_197_022);
    let found = cohort[1];
    let prefix = cohort[0];
    let cohort_before = discover_offer_cohort_snapshot(&engine);
    let found_generation = engine.state.zone_change_generation[&found];
    let announcement = discover_cast_announcement(&mut engine, found, 1);
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(announcement),
                    ..Default::default()
                })),
            },
        )
        .expect("begin the Discover cast transaction");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Discover cast awaits payment")
        .transaction_id;
    engine
        .apply_scenario_command(2, &concede())
        .expect("an unrelated player concedes while Discover cast payment is open");

    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .map(|cast| cast.transaction_id),
        Some(transaction_id),
        "the transaction remains available to cancel"
    );
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(rv1::CancelSpellCast {
                    transaction_id,
                })),
            },
        )
        .expect("cancel returns the discovered object to its original exile incarnation");

    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.objects[&found].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&found],
        found_generation
    );
    assert_eq!(engine.state.objects[&prefix].zone, Zone::Exile);
    assert_eq!(discover_offer_cohort_snapshot(&engine), cohort_before);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
        )
        .expect("the restored offer can be declined and cleanup completes");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&found].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&prefix].zone, Zone::Library);
}

#[test]
fn discover_cast_observers_preserve_random_bottom_privacy_and_physical_cohort() {
    let (mut engine, chimil, cohort) = discover_offer_with_cohort(
        1_197_016,
        &[
            "forest",
            "portal_to_phyrexia",
            "mountain",
            "island",
            "lightning_bolt",
        ],
    );
    let found = *cohort.last().expect("Discover found a spell");
    let expected_cohort = cohort.clone();
    let cohort_before = discover_offer_cohort_snapshot(&engine);
    assert_eq!(
        cohort_before
            .iter()
            .map(|(object_id, _)| *object_id)
            .collect::<Vec<_>>(),
        expected_cohort,
        "the continuation retains the exact physical library cohort in reveal order"
    );
    let generations_before = cohort_before
        .iter()
        .copied()
        .collect::<std::collections::BTreeMap<_, _>>();

    let observer_one = inject_permanent_on_battlefield(&mut engine, 0, "firebrand_archer");
    let observer_two = inject_permanent_on_battlefield(&mut engine, 0, "firebrand_archer");
    let announcement = discover_cast_announcement(&mut engine, found, 1);
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(announcement),
                    ..Default::default()
                })),
            },
        )
        .expect("begin the discovered cast");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Discover spell awaits payment")
        .transaction_id;
    let commit = rv1::CommitSpellCast {
        transaction_id,
        ..Default::default()
    };
    let preview = engine.preview_payment_scenario(
        0,
        &rv1::PreviewPayment {
            transaction_id,
            revision: engine.state.command_index,
            commit_spell_cast: Some(commit),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    let cast_batch = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("commit the free Discover cast and complete its parent effect");

    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&chimil].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&found].zone, Zone::Stack);
    assert_eq!(
        engine.state.zone_change_generation[&found],
        generations_before[&found] + 1,
        "the found physical card moves once from exile onto the stack"
    );
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.id == found)
            .count(),
        1,
        "the exact exiled object is the committed spell"
    );

    let returned = &cohort[..cohort.len() - 1];
    for object_id in returned {
        assert_eq!(engine.state.objects[object_id].zone, Zone::Library);
        assert_eq!(
            engine.state.zone_change_generation[object_id],
            generations_before[object_id] + 1,
            "each exact prefix object returns to the library once"
        );
    }
    let public_library_move_order = cast_batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved))
                if moved.destination == rv1::permanent_moved::Destination::Library as i32 =>
            {
                Some(moved.object_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        public_library_move_order, returned,
        "public move events retain reveal order, independent of randomized bottom order"
    );

    let actual_bottom = engine.state.players[0]
        .library
        .iter()
        .skip(engine.state.players[0].library.len() - returned.len())
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        actual_bottom
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>(),
        returned
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>(),
        "only the exact non-hit cohort is randomized onto the bottom"
    );
    assert_ne!(
        actual_bottom, returned,
        "the fixed seed exercises a changed random order rather than the reveal order"
    );

    let view = discover_zone_view(&cast_batch);
    let owner_view = discover_player_view(view, 0);
    assert!(!owner_view.private_zones_unchanged);
    assert_eq!(
        owner_view
            .library_cards
            .iter()
            .map(|card| card.object_id)
            .collect::<Vec<_>>(),
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        "the owner receives their actual randomized library order"
    );
    let opponent_view = discover_player_view(view, 1);
    assert!(opponent_view.private_zones_unchanged);
    assert!(opponent_view.hand_cards.is_empty());
    assert!(opponent_view.library_cards.is_empty());
    assert!(opponent_view
        .hand_cards
        .iter()
        .all(|card| !cohort.contains(&card.object_id)));
    assert!(opponent_view
        .library_cards
        .iter()
        .all(|card| !cohort.contains(&card.object_id)));

    let pending_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("both Firebrand Archers observed the cast and require ordering");
    assert_eq!(pending_order.deciding_player, 0);
    assert_eq!(pending_order.candidates.len(), 2);
    assert_eq!(
        pending_order
            .candidates
            .iter()
            .map(|trigger| trigger.source_permanent_id)
            .collect::<std::collections::HashSet<_>>(),
        [observer_one, observer_two]
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
    );
    answer_trigger_order_in_engine_order(&mut engine);
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.stack.iter().any(|item| item.id == found));
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered
                && [Some(observer_one), Some(observer_two)].contains(&item.source_permanent_id))
            .count(),
        2,
        "the observer triggers remain available on the stack after Discover resumes"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn discover_commander_choice_is_reissued_after_observer_returns_and_reconnects() {
    for (return_count, seed) in [(1, 1_197_015), (2, 1_197_016)] {
        let (mut engine, chimil, commander, forest, steels) =
            discover_commander_observer_engine(seed, return_count);
        let suspended_parent_id = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Discover commander choice before observer return")
            .resolution_prompt_id
            .get();
        assert_ne!(suspended_parent_id, 0);

        engine
            .apply_scenario_command(1, &concede())
            .expect("P1 concession returns the exiled Sculpting Steels");
        assert!(engine.state.players[1].has_lost);

        let observer_batches = resolve_discover_observer_returns(&mut engine, chimil);
        assert_eq!(
            observer_batches
                .iter()
                .any(|batch| batch.events.iter().any(|event| {
                    matches!(
                        &event.ev,
                        Some(Ev::ResolutionChoiceRequired(choice))
                            if choice.choice_kind
                                == rv1::ChoiceKind::SimultaneousEntryOrder as i32
                    )
                })),
            return_count == 2,
            "two simultaneous returns require timestamp ordering"
        );

        let restored = observer_batches
            .iter()
            .rev()
            .find_map(discover_commander_branch_offer)
            .unwrap_or_else(|| panic!("Discover commander choice was not reissued after {return_count} observer returns"));
        assert_ne!(restored.resolution_prompt_id, suspended_parent_id);
        assert_eq!(restored.deciding_player_id, 0);
        assert_eq!(restored.resolution_branches.len(), 2);
        assert_eq!(restored.resolution_branches[0].label, "Command zone");
        assert_eq!(restored.resolution_branches[1].label, "Hand");

        let reconnect = engine.initial_response_batch();
        let replayed = discover_commander_branch_offer(&reconnect)
            .expect("reconnect snapshot republishes the parked Discover commander choice");
        assert_eq!(
            replayed.resolution_prompt_id, restored.resolution_prompt_id,
            "reconnect republishes the same logical offer ID"
        );
        assert_eq!(replayed.deciding_player_id, restored.deciding_player_id);
        assert_eq!(replayed.source_object_id, restored.source_object_id);
        assert_eq!(replayed.resolution_branches, restored.resolution_branches);

        let stale_answer = RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                resolution_prompt_id: suspended_parent_id,
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: 0,
                ..Default::default()
            })),
        };
        let state_before_stale_parent = format!("{:?}", engine.state);
        assert!(engine.apply_command(0, &stale_answer).is_err());
        assert_eq!(format!("{:?}", engine.state), state_before_stale_parent);

        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                        resolution_prompt_id: restored.resolution_prompt_id,
                        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                        selected_branch_index: 0,
                        ..Default::default()
                    })),
                },
            )
            .expect("choose the command-zone replacement after observer returns");
        while let Some((deciding_player, presentation)) = engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| (pending.deciding_player, pending.presentation.clone()))
        {
            if presentation.choice_kind != ChoiceKind::LegendKeep {
                break;
            }
            assert!(presentation.candidates.contains(&chimil));
            engine
                .apply_scenario_command(deciding_player, &submit_resolution_choice(vec![chimil]))
                .expect("keep the original Chimil after Sculpting Steel copies it");
        }
        assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
        assert_eq!(engine.state.objects[&forest].zone, Zone::Library);
        assert!(steels
            .iter()
            .all(|steel| engine.state.objects[steel].zone == Zone::Graveyard));
        assert!(steels
            .iter()
            .all(|steel| engine.state.players[0].graveyard.contains(steel)));
        assert!(engine.state.players[0].command_zone.contains(&commander));
        assert!(
            engine.state.pending_resolution.is_none(),
            "unexpected pending resolution after Discover cleanup: {:?}",
            engine.state.pending_resolution
        );
    }
}

#[test]
fn discover_owner_departure_during_observer_returns_retires_both_choice_stages() {
    for (stop_on_commander_choice, seed) in [(false, 1_197_017), (true, 1_197_018)] {
        let (mut engine, source, commander, forest, steel, surviving_artifact) =
            discover_departed_owner_observer_engine(seed, stop_on_commander_choice);
        let p2_life = engine.state.players[2].life;
        let p3_life = engine.state.players[3].life;

        engine
            .apply_scenario_command(1, &concede())
            .expect("P1 departure returns P2's exiled Sculpting Steel");
        assert!(engine.state.players[1].has_lost);
        let copy_source = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("P2 must receive the returning Steel's CopySource choice");
        assert_eq!(copy_source.deciding_player, 2);
        assert_eq!(copy_source.presentation.choice_kind, ChoiceKind::CopySource);
        assert!(copy_source
            .presentation
            .candidates
            .contains(&surviving_artifact));

        engine
            .apply_scenario_command(0, &concede())
            .expect("P0 leaves while P2's return choice is still open");
        assert!(engine.state.players[0].has_lost);
        for object_id in [source, commander, forest] {
            assert!(
                !engine.state.objects.contains_key(&object_id),
                "P0-owned cohort object {object_id} leaves the game"
            );
        }

        let completed = engine
            .apply_scenario_command(2, &submit_resolution_choice(vec![surviving_artifact]))
            .expect("P2 completes the returning Steel's CopySource choice");
        assert_eq!(engine.state.objects[&steel].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&steel].card_id, "sculpting_steel");
        assert_eq!(
            engine.state.objects[&surviving_artifact].zone,
            Zone::Battlefield
        );
        assert!(!completed.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.deciding_player_id == 0
                    && matches!(
                        choice.choice_kind,
                        value if value == ChoiceKind::ResolutionBranch as i32
                            || value == ChoiceKind::SpecialCast as i32
                    )
        )));
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_spell_cast.is_none());
        assert_eq!(engine.state.players[2].life, p2_life - 1);
        assert_eq!(
            engine.state.players[3].life,
            p3_life - 1,
            "the parent effect continues to its survivor-affecting tail after P0 leaves"
        );

        let reconnect = engine.initial_response_batch();
        assert!(!reconnect.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.deciding_player_id == 0
                    && matches!(
                        choice.choice_kind,
                        value if value == ChoiceKind::ResolutionBranch as i32
                            || value == ChoiceKind::SpecialCast as i32
                    )
        )));
        pass_priority_round(&mut engine);
    }
}

#[test]
fn discover_owner_departure_during_active_choice_finishes_parent_tail() {
    for (stop_on_commander_choice, seed) in [(false, 1_197_019), (true, 1_197_020)] {
        let (mut engine, source, commander, forest, _, _) =
            discover_departed_owner_observer_engine(seed, stop_on_commander_choice);
        let survivor_life = [
            engine.state.players[1].life,
            engine.state.players[2].life,
            engine.state.players[3].life,
        ];

        let completed = engine
            .apply_scenario_command(0, &concede())
            .expect("the active Discover choice retires when its owner leaves");
        assert!(engine.state.players[0].has_lost);
        for object_id in [source, commander, forest] {
            assert!(!engine.state.objects.contains_key(&object_id));
        }
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_spell_cast.is_none());
        for (player, before) in [1, 2, 3].into_iter().zip(survivor_life) {
            assert_eq!(
                engine.state.players[player].life,
                before - 1,
                "the resolving parent's tail reaches surviving P{player}"
            );
        }
        for batch in [completed, engine.initial_response_batch()] {
            assert!(!batch.events.iter().any(|event| matches!(
                &event.ev,
                Some(Ev::ResolutionChoiceRequired(choice))
                    if choice.deciding_player_id == 0
                        && (choice.choice_kind == ChoiceKind::ResolutionBranch as i32
                            || choice.choice_kind == ChoiceKind::SpecialCast as i32)
            )));
        }
        pass_priority_round(&mut engine);
    }
}

#[test]
fn discover_declined_commander_hit_offers_its_owner_the_command_zone_replacement() {
    let (mut engine, _chimil, commander) =
        commander_discover_engine(1_197_013, "atraxa,_praetors_voice");
    for _ in 0..32 {
        if engine.state.pending_resolution.is_some() || engine.state.turn_step == TurnStep::Cleanup
        {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .expect("decline casting the discovered commander");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("owner receives the commander replacement choice");
    assert_eq!(pending.deciding_player, 0);
    assert!(pending
        .presentation
        .prompt
        .to_lowercase()
        .contains("command zone"));
    assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
    let replacement = engine
        .apply_scenario_command(0, &submit_discover_branch(0))
        .expect("choose the command zone replacement");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert!(engine.state.players[0].command_zone.contains(&commander));
    assert!(engine.state.players[0]
        .hand
        .iter()
        .all(|object_id| *object_id != commander));
    assert!(engine.state.pending_resolution.is_none());
    assert!(replacement.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved))
            if moved.object_id == commander
                && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Command as i32
    )));
}

#[test]
fn discover_bottoming_prefix_commander_can_enter_command_zone_and_remaining_cards_continue() {
    let (mut engine, _chimil, commander) =
        commander_discover_engine(1_197_014, "kokusho,_the_evening_star");
    let [forest, found] = set_library_top(&mut engine, 0, &["forest", "lightning_bolt"])
        .try_into()
        .expect("land and Discover hit");
    engine.state.players[0]
        .library
        .retain(|object_id| *object_id != commander);
    engine.state.players[0].library.push_front(commander);
    for _ in 0..32 {
        if engine.state.pending_resolution.is_some() || engine.state.turn_step == TurnStep::Cleanup
        {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&found].zone, Zone::Exile);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .expect("decline casting the Discover hit");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("owner receives the commander replacement choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
    let bottomed = engine
        .apply_scenario_command(0, &submit_discover_branch(0))
        .expect("choose command zone and finish Discover cleanup");

    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert_eq!(engine.state.objects[&found].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Library);
    assert!(engine.state.players[0].command_zone.contains(&commander));
    assert!(engine.state.pending_resolution.is_none());
    let library_moves = bottomed
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved))
                if moved.destination
                    == tricerules_proto::ruled::v1::permanent_moved::Destination::Library
                        as i32 =>
            {
                Some(moved.object_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(library_moves, [forest]);
    assert_eq!(engine.state.players[0].library.back(), Some(&forest));
}

#[test]
fn discover_exiles_through_first_qualifying_nonland_and_decline_random_bottoms_the_rest() {
    let (mut engine, chimil, land, over_limit, found) = discover_offer(1_197_005);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&chimil].zone, Zone::Battlefield);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.candidates, [found]);
    assert_eq!(engine.state.objects[&land].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&over_limit].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&found].zone, Zone::Exile);

    let declined = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .expect("decline Discover's cast offer");
    assert_eq!(engine.state.objects[&found].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&land].zone, Zone::Library);
    assert_eq!(engine.state.objects[&over_limit].zone, Zone::Library);
    assert!(engine.state.pending_resolution.is_none());
    let library_moves = declined
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved))
                if moved.destination
                    == tricerules_proto::ruled::v1::permanent_moved::Destination::Library
                        as i32 =>
            {
                Some(moved.object_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(library_moves, [land, over_limit]);
    let library_set = engine.state.players[0]
        .library
        .iter()
        .rev()
        .take(2)
        .copied()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(library_set, [land, over_limit].into_iter().collect());
}

#[test]
fn discover_casts_the_hit_for_free_then_random_bottoms_only_the_remaining_prefix() {
    let (mut engine, chimil, land, over_limit, found) = discover_offer(1_197_006);
    assert_eq!(engine.state.objects[&land].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&over_limit].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&found].zone, Zone::Exile);
    let cast_action = engine.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == found)
        .cloned()
        .expect("Discover publishes the qualifying spell face");
    assert_eq!(cast_action.face_index, 0);
    assert_eq!(cast_action.cost, "");
    let cast = tricerules_proto::ruled::v1::CastSpell {
        source: Some(exile_cast_source(
            found,
            engine.state.zone_change_generation[&found],
        )),
        face_index: cast_action.face_index,
        cast_method: cast_action.cast_method,
        casting_permission_id: cast_action.casting_permission_id,
        targets: target_player(1),
        ..Default::default()
    };
    let choice = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::CastSpell as i32,
            cast_spell: Some(cast),
            ..Default::default()
        })),
    };
    let cast_batch = engine
        .apply_scenario_command(0, &choice)
        .expect("cast the Discover hit without paying its mana cost");
    assert_eq!(engine.state.objects[&found].zone, Zone::Stack);
    assert_eq!(engine.state.stack.last().unwrap().id, found);
    assert_eq!(engine.state.objects[&chimil].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&land].zone, Zone::Library);
    assert_eq!(engine.state.objects[&over_limit].zone, Zone::Library);
    assert!(engine.state.pending_resolution.is_none());
    let library_moves = cast_batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved))
                if moved.destination
                    == tricerules_proto::ruled::v1::permanent_moved::Destination::Library
                        as i32 =>
            {
                Some(moved.object_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(library_moves, [land, over_limit]);
}
