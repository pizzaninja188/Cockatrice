//! Draft-only distinguishing fixtures for retained cohorts and otherwise uncommon entry pauses.
use super::*;

const COST: &str = r#"(id:"entry_cost_probe",name:"Entry Cost Probe",face_id:"entry_cost_probe",types:["Artifact"],
    static_abilities:[(ability_id:"entry",presentation:Fallback,definition:EntersTapped(affected:Self_,unless_cost:Some(PayLife(amount:2))))])"#;
const REVEAL: &str = r#"(id:"entry_reveal_probe",name:"Entry Reveal Probe",face_id:"entry_reveal_probe",types:["Artifact"],
    static_abilities:[(ability_id:"entry",presentation:Fallback,definition:EntersTapped(affected:Self_,unless_cost:Some(RevealFromHand(filter:(card_type:Some(Land))))))])"#;
const REPLACEMENT: &str = r#"(id:"entry_order_probe",name:"Entry Order Probe",face_id:"entry_order_probe",types:["Artifact"],
    static_abilities:[(ability_id:"first",presentation:Fallback,definition:EntersTapped(affected:Self_)),
    (ability_id:"second",presentation:Fallback,definition:EntersTapped(affected:Self_))])"#;
const CREATURE: &str = r#"(id:"creature_cohort_probe",name:"Creature Cohort Probe",face_id:"creature_cohort_probe",mana_cost:"{3}{R}{R}",types:["Sorcery"],spell_effect:[
    ExileGraveyards(players:All,filter:Some((card_type:Some(Creature))),capture_exile_cohort:Some("original_graveyards")),
    SacrificeAll(players:All,filter:(kind:AnyPermanent,permanent_types:[Creature])),
    ReturnExiledCohortToOwnersBattlefield(cohort_id:"original_graveyards")])"#;

fn fresh() -> GameEngine {
    let instant = include_str!("../../../../tricerules-cards/data/scrap_mastery.ron")
        .replace("scrap_mastery", "instant_cohort_probe")
        .replace("Scrap Mastery", "Instant Cohort Probe")
        .replace("[\"Sorcery\"]", "[\"Instant\"]");
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../../tricerules-cards/data/scrap_mastery.ron"),
                include_str!("../../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../../tricerules-cards/data/sol_ring.ron"),
                include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../../tricerules-cards/data/black_vise.ron"),
                include_str!("../../../../tricerules-cards/data/phyrexian_metamorph.ron"),
                COST,
                REVEAL,
                REPLACEMENT,
                CREATURE,
                &instant,
            ],
            &[],
        )
        .unwrap(),
    ));
    let deck = EngineDeck {
        mainboard: vec!["mountain".into(); 20],
        ..Default::default()
    };
    let mut engine = GameEngine::new_with_registry(
        509_700,
        &[4, 9, 27],
        20,
        Some(vec![deck; 3]),
        true,
        registry,
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine
}

fn place(engine: &mut GameEngine, seat: usize, card: &str, zone: Zone) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let player = engine.state.players[seat].id;
    let object = new_object_from_card(
        oid,
        player,
        card,
        zone,
        engine.registry.get(card).unwrap().primary_face(),
    );
    engine.state.objects.insert(oid, object);
    engine.state.zone_change_generation.insert(oid, 0);
    match zone {
        Zone::Hand => engine.state.players[seat].hand.push(oid),
        Zone::Graveyard => engine.state.players[seat].graveyard.push(oid),
        Zone::Battlefield => {
            engine.state.players[seat].battlefield.push(oid);
            engine.emit_static_abilities_on_enter(oid);
        }
        Zone::Exile => engine.state.players[seat].exile.push(oid),
        _ => unreachable!(),
    }
    oid
}

fn answer(ids: Vec<ObjectId>, decision: rv1::ResolutionChoiceDecision) -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                chosen_object_ids: ids,
                decision: decision as i32,
                ..Default::default()
            },
        )),
    }
}

fn paid(engine: &mut GameEngine, card: &str) -> ObjectId {
    let oid = place(engine, 0, card, Zone::Hand);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == oid)
        .unwrap();
    engine.state.players[0].mana_pool.colorless = 3;
    engine.state.players[0].mana_pool.red = 2;
    engine
        .apply_command(
            4,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(slot as u32)),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    for _ in 0..3 {
        engine
            .apply_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(Default::default())),
                },
            )
            .unwrap();
    }
    oid
}

fn concede(engine: &mut GameEngine, actor: PlayerId) {
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(Default::default())),
            },
        )
        .unwrap();
}

fn decline_or_order(engine: &mut GameEngine) {
    for _ in 0..8 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            return;
        };
        let actor = pending.deciding_player;
        let (ids, decision) = match pending.continuation {
            ResolutionContinuation::EntryCost { .. } => {
                (vec![], rv1::ResolutionChoiceDecision::Decline)
            }
            ResolutionContinuation::EntryReveal { .. } => {
                (vec![], rv1::ResolutionChoiceDecision::Unspecified)
            }
            ResolutionContinuation::EntryReplacement { .. } => (
                vec![pending.presentation.candidates[0]],
                rv1::ResolutionChoiceDecision::Unspecified,
            ),
            ResolutionContinuation::SimultaneousEntryOrder { .. } => (
                pending.presentation.candidates.clone(),
                rv1::ResolutionChoiceDecision::Unspecified,
            ),
            ResolutionContinuation::MassSacrificeGraveyardOrder { .. } => (
                pending.presentation.candidates.clone(),
                rv1::ResolutionChoiceDecision::Unspecified,
            ),
            _ => panic!("unexpected draft entry choice"),
        };
        engine.apply_command(actor, &answer(ids, decision)).unwrap();
    }
    panic!("draft resolution did not complete");
}

#[test]
fn retained_cohort_each_entry_pause_skips_departed_current_entrant() {
    for card in [
        "entry_cost_probe",
        "entry_reveal_probe",
        "entry_order_probe",
    ] {
        let mut engine = fresh();
        let departed = place(&mut engine, 1, card, Zone::Graveyard);
        let survivor = place(&mut engine, 2, "sol_ring", Zone::Graveyard);
        let spell = paid(&mut engine, "scrap_mastery");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            9
        );
        concede(&mut engine, 9);
        assert!(engine.state.pending_resolution.is_none());
        assert!(!engine.state.objects.contains_key(&departed));
        assert_eq!(engine.state.objects[&survivor].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    }
}

#[test]
fn retained_cohort_each_entry_pause_preserves_surviving_current_entrant() {
    for card in [
        "entry_cost_probe",
        "entry_reveal_probe",
        "entry_order_probe",
    ] {
        let mut engine = fresh();
        let survivor = place(&mut engine, 1, card, Zone::Graveyard);
        let departed = place(&mut engine, 2, "sol_ring", Zone::Graveyard);
        let spell = paid(&mut engine, "scrap_mastery");
        let kind = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind;
        concede(&mut engine, 27);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            kind
        );
        decline_or_order(&mut engine);
        assert_eq!(engine.state.objects[&survivor].zone, Zone::Battlefield);
        assert!(engine.state.objects[&survivor].tapped);
        assert!(!engine.state.objects.contains_key(&departed));
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    }
}

#[test]
fn retained_cohort_real_entry_life_payment_is_not_an_intermediate_loss() {
    let mut engine = fresh();
    engine.state.players[0].life = 2;
    place(&mut engine, 0, "entry_cost_probe", Zone::Graveyard);
    place(&mut engine, 1, "entry_reveal_probe", Zone::Graveyard);
    paid(&mut engine, "scrap_mastery");
    engine
        .apply_command(
            4,
            &answer(vec![], rv1::ResolutionChoiceDecision::SelectBranch),
        )
        .unwrap();
    assert_eq!(engine.state.players[0].life, 0);
    assert!(!engine.state.players[0].has_lost);
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        ResolutionContinuation::EntryReveal { .. }
    ));
    concede(&mut engine, 27);
    assert!(!engine.state.players[0].has_lost);
    assert!(!engine.state.is_terminal());
    decline_or_order(&mut engine);
    assert!(
        engine.state.players[0].has_lost,
        "the life SBA happens after the complete resolution"
    );
}

#[test]
fn retained_cohort_concession_abandons_stale_current_entry_in_every_participating_pause() {
    for card in [
        "entry_cost_probe",
        "entry_reveal_probe",
        "entry_order_probe",
        "black_vise",
        "phyrexian_metamorph",
    ] {
        let mut engine = fresh();
        place(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
        let entrant = place(&mut engine, 1, card, Zone::Graveyard);
        let spell = paid(&mut engine, "scrap_mastery");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            9
        );
        *engine
            .state
            .zone_change_generation
            .get_mut(&entrant)
            .unwrap() += 1;
        let generation = engine.state.zone_change_generation[&entrant];
        // Explicitly queued independent CR610.3 work must survive abandonment of this proposal.
        let owed = place(&mut engine, 0, "grizzly_bears", Zone::Exile);
        engine.state.pending_immediate_observer_actions.push(
            ImmediateObserverAction::ReturnExiledObject {
                exiled: TriggerObjectRef {
                    object_id: owed,
                    zone_change_generation: 0,
                    controller_at_event: 4,
                },
            },
        );
        let batch = engine
            .apply_command(
                27,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(Default::default())),
                },
            )
            .unwrap();
        assert!(
            engine.state.pending_resolution.is_none(),
            "{card}: stale entry must not leave an unusable choice"
        );
        assert!(engine.state.pending_replacement_event.is_none());
        assert_eq!(engine.state.objects[&entrant].zone, Zone::Exile);
        assert_eq!(engine.state.zone_change_generation[&entrant], generation);
        assert_eq!(engine.state.objects[&owed].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::StackResolved(_))))
                .count(),
            1
        );
    }
}

#[test]
fn retained_cohort_creature_shape_uses_the_same_runtime_without_admitting_living_death() {
    let mut engine = fresh();
    assert!(engine.registry.get("living_death").is_none());
    let originals = [
        place(&mut engine, 0, "grizzly_bears", Zone::Graveyard),
        place(&mut engine, 2, "grizzly_bears", Zone::Graveyard),
    ];
    let victim = place(&mut engine, 1, "grizzly_bears", Zone::Battlefield);
    let artifact = place(&mut engine, 2, "sol_ring", Zone::Battlefield);
    let existing = place(&mut engine, 1, "grizzly_bears", Zone::Exile);
    let spell = paid(&mut engine, "creature_cohort_probe");
    decline_or_order(&mut engine);
    for oid in originals {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    }
    assert_eq!(engine.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&existing].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn retained_cohort_opponent_entry_departure_skips_entrant_or_preserves_branch_identity() {
    let mut engine = fresh();
    let departed = place(&mut engine, 1, "black_vise", Zone::Graveyard);
    let survivor = place(&mut engine, 2, "sol_ring", Zone::Graveyard);
    let spell = paid(&mut engine, "scrap_mastery");
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        ResolutionContinuation::EntryChooseOpponent { .. }
    ));
    concede(&mut engine, 9);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.objects.contains_key(&departed));
    assert_eq!(engine.state.objects[&survivor].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);

    let mut engine = fresh();
    let entrant = place(&mut engine, 1, "black_vise", Zone::Graveyard);
    place(&mut engine, 2, "sol_ring", Zone::Graveyard);
    let spell = paid(&mut engine, "scrap_mastery");
    concede(&mut engine, 27);
    let before = format!("{:?}", engine.state);
    let branch = |index| RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: index,
                ..Default::default()
            },
        )),
    };
    assert!(
        engine.apply_command(9, &branch(1)).is_err(),
        "the departed opponent's frozen branch remains invalid"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    engine.apply_command(9, &branch(0)).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn retained_cohort_bounced_original_is_not_rebound_to_the_same_physical_id() {
    let mut engine = fresh();
    let original = place(&mut engine, 2, "sol_ring", Zone::Graveyard);
    place(&mut engine, 1, "sol_ring", Zone::Battlefield);
    place(&mut engine, 1, "entry_order_probe", Zone::Battlefield);
    paid(&mut engine, "scrap_mastery");
    assert_eq!(engine.state.objects[&original].zone, Zone::Exile);
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        original,
        Zone::Hand,
        None,
    )
    .unwrap();
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        original,
        Zone::Exile,
        None,
    )
    .unwrap();
    let generation = engine.state.zone_change_generation[&original];
    decline_or_order(&mut engine);
    assert_eq!(engine.state.objects[&original].zone, Zone::Exile);
    assert_eq!(engine.state.zone_change_generation[&original], generation);
}

#[test]
fn retained_cohort_missing_capture_and_duplicate_capture_fail_before_moving_cards() {
    let mut engine = fresh();
    place(&mut engine, 2, "sol_ring", Zone::Graveyard);
    let mut item = engine.observer_return_item(u32::MAX, 4);
    item.card_id = "scrap_mastery".into();
    item.ability_text = None;
    let (effects, label) = engine.build_resolution_effects(&item);
    let before = format!("{:?}", engine.state);
    assert!(engine
        .run_effect_list_with_previous(
            &item,
            &label,
            effects,
            2,
            Default::default(),
            &mut Vec::new()
        )
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    item.exiled_cohorts.insert(
        tricerules_cards::ExiledCohortId::new("original_graveyards").unwrap(),
        Vec::new(),
    );
    let (effects, label) = engine.build_resolution_effects(&item);
    assert!(engine
        .run_effect_list_with_previous(
            &item,
            &label,
            effects,
            0,
            Default::default(),
            &mut Vec::new()
        )
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn retained_cohort_apnap_is_independent_of_the_caster_in_instant_shape_fixture() {
    let mut engine = fresh();
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 0;
    for seat in 0..3 {
        place(&mut engine, seat, "sol_ring", Zone::Graveyard);
        place(&mut engine, seat, "sol_ring", Zone::Battlefield);
        place(&mut engine, seat, "entry_order_probe", Zone::Battlefield);
    }
    paid(&mut engine, "instant_cohort_probe");
    for actor in [9, 27, 4] {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert!(matches!(
            pending.continuation,
            ResolutionContinuation::MassSacrificeGraveyardOrder { .. }
        ));
        assert_eq!(pending.deciding_player, actor);
        engine
            .apply_command(
                actor,
                &answer(
                    pending.presentation.candidates.clone(),
                    rv1::ResolutionChoiceDecision::Unspecified,
                ),
            )
            .unwrap();
    }
    assert!(engine.state.pending_resolution.is_none());
    for player in &engine.state.players {
        assert_eq!(player.battlefield.len(), 1);
        assert_eq!(
            engine.state.objects[&player.battlefield[0]].controller,
            player.id
        );
    }
}
