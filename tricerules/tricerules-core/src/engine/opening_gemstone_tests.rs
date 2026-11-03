use super::*;

// Prepared opening fixture with a synthetic exact opening clause, not card admission
// or a claim of a legal constructed deck. Strict actual-card startup is separate.
const SOURCE: &str = r#"(id:"synthetic_caverns",name:"Synthetic Caverns",face_id:"synthetic_caverns",
    supertypes:["Legendary"],types:["Land"],static_abilities:[(
        ability_id:"opening",presentation:Fallback,definition:GemstoneCavernsOpening)],
    activated_abilities:[(ability_id:"mana",presentation:Fallback,costs:[Tap],effect:[ProduceMana(
        options:[(c:1)],conditional:Some((condition:SourceCounterCount(counter:Luck,min:Some(1)),
        options:[(w:1),(u:1),(b:1),(r:1),(g:1)])))])])"#;
const DOUBLER: &str = r#"(id:"counter_doubler",name:"Counter Doubler",face_id:"counter_doubler",
    types:["Enchantment"],static_abilities:[(ability_id:"double",presentation:Fallback,definition:DoubleCountersYouWouldPut)])"#;

#[test]
fn gemstone_actual_new_battlefield_incarnation_does_not_reuse_opening_luck() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        705_121,
        &[0, 1],
        20,
        Some(vec![vec!["gemstone_caverns".into(); 40]; 2]),
        true,
    )
    .unwrap();
    let id = engine.state.players[0].library[0];
    super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        id,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    engine
        .state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::Luck, 1);
    let previous = engine.state.zone_change_generation[&id];
    super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        id,
        Zone::Exile,
        None,
    )
    .unwrap();
    super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        id,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    assert!(engine.state.zone_change_generation[&id] > previous);
    assert_eq!(
        engine.state.objects[&id].counter_count(CounterKind::Luck),
        0
    );
}

fn fixture(players: &[PlayerId]) -> GameEngine {
    fixture_with_source(players, SOURCE)
}

fn fixture_with_source(players: &[PlayerId], source: &str) -> GameEngine {
    let registry = CardRegistry::from_chunks_and_tokens(&[source, DOUBLER], &[]).unwrap();
    GameEngine::new_with_registry(
        705_101,
        players,
        20,
        Some(vec![
            EngineDeck {
                mainboard: vec!["synthetic_caverns".into(); 40],
                commanders: vec![],
            };
            players.len()
        ]),
        false,
        Box::leak(Box::new(registry)),
    )
    .unwrap()
}

#[test]
fn gemstone_opening_entry_replacement_announcement_never_reveals_the_exile_choice() {
    let copying=SOURCE.replace("definition:GemstoneCavernsOpening)",
        "definition:GemstoneCavernsOpening),(ability_id:\"copy\",presentation:Fallback,definition:EntersAsCopy(filter:(kind:AnyPermanent),artifact_in_addition:false))");
    let mut engine = fixture_with_source(&[0, 1, 2], &copying);
    keep_all(&mut engine, 0);
    install_doublers(&mut engine, 1);
    let cards = engine.opening_hand_cards(1);
    let source = cards[0].object.as_ref().unwrap().object_id;
    let exile = cards[1].object.as_ref().unwrap().object_id;
    begin_first(&mut engine, 1);
    assert!(matches!(
        engine.state.pending_replacement_event,
        Some(super::replacement::PendingReplacementEvent::BattlefieldEntry(_))
    ));
    let reveal = super::reveals::active_reveals(&engine);
    assert_eq!(reveal.len(), 1);
    assert_eq!(reveal[0].cards.len(), 1);
    assert_eq!(reveal[0].cards[0].object_id, source);
    assert!(!reveal[0].cards.iter().any(|card| card.object_id == exile));
    command(
        &mut engine,
        1,
        rv1::ruled_command::Cmd::Concede(rv1::Concede {}),
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(super::reveals::active_reveals(&engine).is_empty());
    assert_eq!(
        engine
            .state
            .opening
            .as_ref()
            .unwrap()
            .hand_window
            .as_ref()
            .unwrap()
            .actor,
        2
    );
}

fn install_doublers(engine: &mut GameEngine, actor: PlayerId) {
    install_counter_sources(engine, actor, 2);
}
fn install_counter_sources(engine: &mut GameEngine, actor: PlayerId, count: usize) {
    for _ in 0..count {
        let id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        let mut object = engine.state.objects.values().next().unwrap().clone();
        object.id = id;
        object.card_id = "counter_doubler".into();
        object.owner = actor;
        object.controller = actor;
        object.base_controller = actor;
        object.zone = Zone::Battlefield;
        engine.state.objects.insert(id, object);
        let index = engine.state.player_idx(actor).unwrap();
        engine.state.players[index].battlefield.push(id);
        engine.emit_static_abilities_on_enter(id);
    }
}

fn begin_first(engine: &mut GameEngine, actor: PlayerId) -> RuledEventBatch {
    let cards = engine.opening_hand_cards(actor);
    command(
        engine,
        actor,
        rv1::ruled_command::Cmd::BeginWithGemstoneCaverns(rv1::BeginWithGemstoneCaverns {
            source: cards[0].object,
            exile: cards.get(1).and_then(|card| card.object),
        }),
    )
}

#[test]
fn gemstone_opening_actor_departure_retires_parked_entry_before_next_player() {
    let mut engine = fixture(&[0, 1, 2]);
    keep_all(&mut engine, 0);
    install_doublers(&mut engine, 1);
    begin_first(&mut engine, 1);
    assert!(engine.pending_opening_hand_entry().is_some());
    command(
        &mut engine,
        1,
        rv1::ruled_command::Cmd::Concede(rv1::Concede {}),
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert_eq!(
        engine
            .state
            .opening
            .as_ref()
            .unwrap()
            .hand_window
            .as_ref()
            .unwrap()
            .actor,
        2
    );
    assert!(!engine.opening_gemstone_cards(2).is_empty());
    begin_first(&mut engine, 2);
}

#[test]
fn gemstone_opening_departed_replacement_owner_refreshes_a_surviving_entry() {
    let mut engine = fixture(&[0, 1, 2]);
    keep_all(&mut engine, 0);
    install_doublers(&mut engine, 1);
    let support = engine.state.players[1].battlefield.clone();
    for id in support {
        engine.state.objects.get_mut(&id).unwrap().owner = 0;
    }
    let source = engine.opening_hand_cards(1)[0]
        .object
        .as_ref()
        .unwrap()
        .object_id;
    begin_first(&mut engine, 1);
    assert!(engine.state.pending_resolution.is_some());
    command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::Concede(rv1::Concede {}),
    );
    assert!(
        engine.state.pending_resolution.is_none(),
        "vanished replacements must not strand the opening transaction"
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source]
            .counters
            .get(&CounterKind::Luck),
        Some(&1)
    );
}

#[test]
fn gemstone_opening_departure_preserves_an_already_applied_counter_replacement() {
    let mut engine = fixture(&[0, 1, 2]);
    keep_all(&mut engine, 0);
    install_counter_sources(&mut engine, 1, 3);
    let support = engine.state.players[1].battlefield.clone();
    for &id in &support[1..] {
        engine.state.objects.get_mut(&id).unwrap().owner = 0;
    }
    let source = engine.opening_hand_cards(1)[0]
        .object
        .as_ref()
        .unwrap()
        .object_id;
    begin_first(&mut engine, 1);
    let Some(super::replacement::PendingReplacementEvent::Counters(batch)) =
        &engine.state.pending_replacement_event
    else {
        panic!("counter choice required")
    };
    let chosen=batch.applications.iter().find(|application|matches!(&application.replacement,
        super::replacement::CounterPlacementReplacement::Battlefield {instance,..} if instance.source_object_id==support[0])).unwrap().choice_id;
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    command(
        &mut engine,
        1,
        rv1::ruled_command::Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            resolution_prompt_id: prompt,
            chosen_object_ids: vec![chosen],
            ..Default::default()
        }),
    );
    assert!(engine.state.pending_resolution.is_some());
    command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::Concede(rv1::Concede {}),
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Luck),
        2,
        "accepted doubling survives; departed replacements do not apply or restart the placement"
    );
}

#[test]
fn gemstone_opening_parked_counter_choices_preserve_private_receipts_and_resume_only_entry() {
    let mut engine = fixture(&[0, 1, 2]);
    keep_all(&mut engine, 0);
    install_doublers(&mut engine, 1);
    let cards = engine.opening_hand_cards(1);
    let source = cards[0].object.unwrap();
    let exile = cards[1].object.unwrap();
    let batch = begin_first(&mut engine, 1);
    let reveals = super::reveals::active_reveals(&engine);
    assert_eq!(
        reveals.len(),
        1,
        "accepted Caverns remains revealed while entry parks"
    );
    assert_eq!(reveals[0].cards.len(), 1);
    assert_eq!(reveals[0].cards[0].object_id, source.object_id);
    assert!(!reveals[0]
        .cards
        .iter()
        .any(|card| card.object_id == exile.object_id));
    assert_eq!(engine.state.objects[&source.object_id].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&exile.object_id].zone, Zone::Hand);
    assert!(batch.legal_by_player[&1]
        .opening
        .as_ref()
        .unwrap()
        .gemstone_caverns
        .is_empty());
    assert!(
        !batch.legal_by_player[&1]
            .opening
            .as_ref()
            .unwrap()
            .can_finish_hand_actions
    );
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::FinishOpeningHandActions(
                    rv1::FinishOpeningHandActions {}
                ))
            }
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    // A different participant's departure must preserve the accepted transaction.
    command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::Concede(rv1::Concede {}),
    );
    assert!(engine.pending_opening_hand_entry().is_some());
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        let answer = rv1::SubmitResolutionChoice {
            resolution_prompt_id: pending.resolution_prompt_id.get(),
            chosen_object_ids: vec![pending.presentation.candidates[0]],
            ..Default::default()
        };
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(
                2,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        answer.clone()
                    ))
                }
            )
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        command(
            &mut engine,
            1,
            rv1::ruled_command::Cmd::SubmitResolutionChoice(answer),
        );
    }
    assert_eq!(
        engine.state.objects[&source.object_id].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&source.object_id]
            .counters
            .get(&CounterKind::Luck),
        Some(&4)
    );
    assert_eq!(engine.state.objects[&exile.object_id].zone, Zone::Exile);
    assert_eq!(
        engine
            .state
            .opening
            .as_ref()
            .unwrap()
            .hand_window
            .as_ref()
            .unwrap()
            .actor,
        1
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn gemstone_opening_rejects_wrong_actor_stale_and_same_card_refs_atomically() {
    let mut engine = fixture(&[0, 1]);
    keep_all(&mut engine, 0);
    let cards = engine.opening_hand_cards(1);
    let source = cards[0].object.unwrap();
    let exile = cards[1].object.unwrap();
    let mut stale = exile;
    stale.zone_change_generation += 1;
    let mut stale_source = source;
    stale_source.zone_change_generation += 1;
    for (actor, source, exile) in [
        (0, source, Some(exile)),
        (1, source, Some(source)),
        (1, source, Some(stale)),
        (1, stale_source, Some(exile)),
        (1, source, None),
    ] {
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::BeginWithGemstoneCaverns(
                        rv1::BeginWithGemstoneCaverns {
                            source: Some(source),
                            exile
                        }
                    ))
                }
            )
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::FinishOpeningHandActions(
                    rv1::FinishOpeningHandActions {}
                ))
            }
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn gemstone_opening_multiple_copies_wait_for_window_end_before_legend_sba_and_never_offer_mana() {
    let mut engine = fixture(&[0, 1, 2]);
    keep_all(&mut engine, 0);
    for _ in 0..3 {
        begin_first(&mut engine, 1);
    }
    assert_eq!(engine.state.players[1].battlefield.len(), 3);
    assert_eq!(engine.state.players[1].hand.len(), 1);
    assert!(
        engine.state.pending_resolution.is_none(),
        "legend SBA waits until all opening actions finish"
    );
    let mut legal = RuledEventBatch::default();
    legal_actions::fill_legal(&mut legal, &engine);
    assert!(legal.legal_by_player[&1].permanent_actions.is_empty());
    for &id in &engine.state.players[1].battlefield {
        let ability = &engine
            .registry
            .get("synthetic_caverns")
            .unwrap()
            .primary_face()
            .activated_abilities[0];
        assert!(!engine.ability_activatable(id, 0, ability));
    }
    begin_first(&mut engine, 1); // sole remaining Caverns enters; no other hand card exists.
    assert!(engine.state.players[1].hand.is_empty());
    assert_eq!(engine.state.players[1].battlefield.len(), 4);
    assert_eq!(
        engine
            .state
            .opening
            .as_ref()
            .unwrap()
            .hand_window
            .as_ref()
            .unwrap()
            .actor,
        2
    );
    assert!(engine.state.pending_resolution.is_none());
    command(
        &mut engine,
        2,
        rv1::ruled_command::Cmd::FinishOpeningHandActions(rv1::FinishOpeningHandActions {}),
    );
    assert!(engine.state.opening.is_none());
    assert!(
        engine.state.pending_resolution.is_some(),
        "first priority boundary now owes legend choice"
    );
}

fn command(
    engine: &mut GameEngine,
    actor: PlayerId,
    cmd: rv1::ruled_command::Cmd,
) -> RuledEventBatch {
    engine
        .apply_command(actor, &RuledCommand { cmd: Some(cmd) })
        .unwrap()
}

fn keep_all(engine: &mut GameEngine, starter: PlayerId) {
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    command(
        engine,
        chooser,
        rv1::ruled_command::Cmd::ChooseStartingPlayer(rv1::ChooseStartingPlayer {
            starting_player_id: starter,
        }),
    );
    while let Some(actor) = engine
        .state
        .opening
        .as_ref()
        .and_then(|op| op.mulligan_actor)
    {
        command(
            engine,
            actor,
            rv1::ruled_command::Cmd::Mulligan(rv1::MulliganDecision { keep: true }),
        );
    }
}

#[test]
fn gemstone_opening_entry_exiles_the_exact_other_card_without_a_land_play_or_cast() {
    let mut engine = fixture(&[0, 1]);
    keep_all(&mut engine, 0);
    let cards = engine.opening_hand_cards(1);
    let source = cards[0].object.unwrap();
    let exile = cards[1].object.unwrap();
    let batch = command(
        &mut engine,
        1,
        rv1::ruled_command::Cmd::BeginWithGemstoneCaverns(rv1::BeginWithGemstoneCaverns {
            source: Some(source),
            exile: Some(exile),
        }),
    );
    let object = &engine.state.objects[&source.object_id];
    assert_eq!(object.zone, Zone::Battlefield);
    assert_eq!(object.counters.get(&CounterKind::Luck), Some(&1));
    assert_eq!(engine.state.objects[&exile.object_id].zone, Zone::Exile);
    assert!(!engine.state.objects[&exile.object_id].face_down);
    assert_eq!(engine.state.players[1].hand.len(), 5);
    assert_eq!(engine.state.lands_played_this_turn, 0);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.opening.is_some());
    assert_eq!(
        batch.legal_by_player[&1]
            .opening
            .as_ref()
            .unwrap()
            .gemstone_caverns
            .len(),
        5
    );
}

#[test]
fn gemstone_original_starter_departure_does_not_disqualify_the_next_live_first_player() {
    for leave_during_hand_actions in [false, true] {
        let mut engine = fixture(&[0, 1, 2, 3]);
        if leave_during_hand_actions {
            keep_all(&mut engine, 0);
        } else {
            let chooser = engine.state.opening.as_ref().unwrap().chooser;
            command(
                &mut engine,
                chooser,
                rv1::ruled_command::Cmd::ChooseStartingPlayer(rv1::ChooseStartingPlayer {
                    starting_player_id: 0,
                }),
            );
        }
        command(
            &mut engine,
            0,
            rv1::ruled_command::Cmd::Concede(rv1::Concede {}),
        );
        while let Some(actor) = engine
            .state
            .opening
            .as_ref()
            .and_then(|op| op.mulligan_actor)
        {
            command(
                &mut engine,
                actor,
                rv1::ruled_command::Cmd::Mulligan(rv1::MulliganDecision { keep: true }),
            );
        }
        let opening = engine.state.opening.as_ref().unwrap();
        assert_eq!(opening.original_starting_player, Some(0));
        assert_eq!(opening.starting_player, Some(1));
        assert_eq!(opening.hand_window.as_ref().unwrap().actor, 1);
        assert_eq!(engine.opening_gemstone_cards(1).len(), 7);
        for actor in [1, 2, 3] {
            command(
                &mut engine,
                actor,
                rv1::ruled_command::Cmd::FinishOpeningHandActions(rv1::FinishOpeningHandActions {}),
            );
        }
        assert!(engine.state.opening.is_none());
        assert_eq!(engine.state.active_player_id(), 1);
    }
}

#[test]
fn gemstone_opening_runtime_waits_for_last_bottom_before_nonstarter_hand_actions() {
    let mut engine = fixture(&[0, 1]);
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    command(
        &mut engine,
        chooser,
        rv1::ruled_command::Cmd::ChooseStartingPlayer(rv1::ChooseStartingPlayer {
            starting_player_id: 0,
        }),
    );
    command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::Mulligan(rv1::MulliganDecision { keep: false }),
    );
    command(
        &mut engine,
        1,
        rv1::ruled_command::Cmd::Mulligan(rv1::MulliganDecision { keep: true }),
    );
    let batch = command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::Mulligan(rv1::MulliganDecision { keep: true }),
    );
    assert_eq!(engine.state.opening.as_ref().unwrap().bottom, Some((0, 1)));
    assert_eq!(
        batch.legal_by_player[&1].opening.as_ref().unwrap().stage,
        rv1::OpeningStage::Bottom as i32
    );
    let batch = command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::PutOpeningHandOnBottom(rv1::PutOpeningHandOnBottom {
            hand_card_index: 0,
        }),
    );
    assert!(
        engine.state.opening.is_some(),
        "opening-hand actions must finish before Upkeep"
    );
    let offer = batch.legal_by_player[&1].opening.as_ref().unwrap();
    assert_eq!(offer.stage, 4, "post-mulligan opening-hand action stage");
    assert_eq!(offer.deciding_player_id, 1);
    assert!(!offer.gemstone_caverns.is_empty());
    assert!(offer.can_finish_hand_actions);
    assert!(batch.legal_by_player[&0]
        .opening
        .as_ref()
        .unwrap()
        .gemstone_caverns
        .is_empty());
    assert!(!batch.events.iter().any(|event| matches!(&event.ev,
        Some(rv1::ruled_event::Ev::PhaseChanged(phase)) if phase.phase_id == rv1::PhaseId::Upkeep as i32)));
    let before = engine.diagnostic_snapshot().unwrap();
    let pass = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
    };
    assert!(engine.apply_command(1, &pass).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn gemstone_opening_rejects_departed_starting_player_before_binding_original_identity() {
    let mut engine = fixture(&[0, 1, 2]);
    command(
        &mut engine,
        0,
        rv1::ruled_command::Cmd::Concede(rv1::Concede {}),
    );
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(
        engine
            .apply_command(
                chooser,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::ChooseStartingPlayer(
                        rv1::ChooseStartingPlayer {
                            starting_player_id: 0
                        }
                    ))
                }
            )
            .is_err(),
        "the original starting player must be a live eligible player"
    );
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}
