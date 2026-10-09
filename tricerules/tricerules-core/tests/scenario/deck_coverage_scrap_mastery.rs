//! Exact paid Scrap Mastery: three simultaneous phases and retained post-exile incarnations.
use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, EffectDuration};
use tricerules_core::Zone;
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::ChoiceKind;

fn setup() -> GameEngine {
    let deck = deck_with("mountain", &["scrap_mastery"]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        509_100,
        &[4, 9, 27],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "scrap_mastery");
    engine
}

fn cast(engine: &mut GameEngine) -> u32 {
    engine.state.players[0].mana_pool.colorless = 3;
    engine.state.players[0].mana_pool.red = 2;
    let slot = hand_index_for_card(engine, 0, "scrap_mastery");
    let spell = engine.state.players[0].hand[slot];
    engine.apply_command(4, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    for _ in 0..engine.state.players.len() {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    spell
}

fn order_all(engine: &mut GameEngine) {
    for _ in 0..12 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            return;
        };
        assert!(
            pending.presentation.ordered,
            "only ordering choices are expected in this fixture"
        );
        let actor = pending.deciding_player;
        let mut choices = pending.presentation.candidates.clone();
        choices.reverse();
        engine
            .apply_command(actor, &submit_resolution_choice(choices))
            .unwrap();
    }
    panic!("resolution did not complete");
}

#[test]
fn scrap_mastery_paid_cast_returns_only_original_artifacts_to_each_owner() {
    let mut engine = setup();
    let original: Vec<_> = (0..3)
        .map(|seat| inject_graveyard_card(&mut engine, seat, "sol_ring"))
        .collect();
    let sacrificed: Vec<_> = (0..3)
        .map(|seat| inject_permanent_on_battlefield(&mut engine, seat, "short_sword"))
        .collect();
    let nonartifact = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let existing_exile = inject_graveyard_card(&mut engine, 2, "mind_stone");
    engine.state.players[2]
        .graveyard
        .retain(|oid| *oid != existing_exile);
    engine.state.players[2].exile.push(existing_exile);
    engine.state.objects.get_mut(&existing_exile).unwrap().zone = Zone::Exile;
    let spell = cast(&mut engine);
    order_all(&mut engine);
    for (seat, oid) in original.iter().enumerate() {
        assert_eq!(engine.state.objects[oid].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[oid].controller,
            engine.state.players[seat].id
        );
        assert_eq!(engine.state.zone_change_generation[oid], 2);
    }
    for oid in sacrificed {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }
    assert_eq!(engine.state.objects[&nonartifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&existing_exile].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn scrap_mastery_retains_exiled_cohort_across_each_owners_sacrifice_order() {
    let mut engine = setup();
    let original: Vec<_> = (0..3)
        .map(|seat| inject_graveyard_card(&mut engine, seat, "mind_stone"))
        .collect();
    for seat in 0..3 {
        inject_permanent_on_battlefield(&mut engine, seat, "short_sword");
        inject_permanent_on_battlefield(&mut engine, seat, "sol_ring");
    }
    let spell = cast(&mut engine);
    for actor in [4, 9, 27] {
        assert!(original
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Exile));
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, actor);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::GraveyardCards);
        let mut ids = pending.presentation.candidates.clone();
        ids.reverse();
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_command(
                if actor == 4 { 9 } else { 4 },
                &submit_resolution_choice(ids.clone())
            )
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        engine
            .apply_command(actor, &submit_resolution_choice(ids))
            .unwrap();
    }
    order_all(&mut engine);
    assert!(original
        .iter()
        .all(|oid| engine.state.objects[oid].zone == Zone::Battlefield));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_last_sacrifice_chooser_concession_resumes_surviving_cohort() {
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    let spell = cast(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );
    engine.apply_command(9, &concede()).unwrap();
    assert!(
        engine.state.pending_resolution.is_none(),
        "departed ordering group must not retain the paused resolution"
    );
    assert_eq!(engine.state.objects[&original].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_future_sacrifice_owner_departure_keeps_current_choice_usable() {
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    for seat in [0, 1] {
        inject_permanent_on_battlefield(&mut engine, seat, "short_sword");
        inject_permanent_on_battlefield(&mut engine, seat, "mind_stone");
    }
    let spell = cast(&mut engine);
    let ids = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .clone();
    engine.apply_command(9, &concede()).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        4
    );
    engine
        .apply_command(4, &submit_resolution_choice(ids))
        .expect("surviving committed arrivals remain orderable");
    order_all(&mut engine);
    assert_eq!(engine.state.objects[&original].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_caster_departure_preserves_committed_survivor_graveyard_order() {
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    let a = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let b = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    let spell = cast(&mut engine);
    let batch = engine.apply_command(4, &concede()).unwrap();
    assert!(
        !batch
            .events
            .iter()
            .any(|event| matches!(event.ev, Some(Ev::PriorityChanged(_)))),
        "no ordinary priority while survivor ordering remains outstanding"
    );
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("already committed surviving arrivals still require owner ordering");
    assert_eq!(pending.deciding_player, 9);
    engine
        .apply_command(9, &submit_resolution_choice(vec![b, a]))
        .unwrap();
    assert_eq!(engine.state.players[1].graveyard, vec![b, a]);
    assert_eq!(
        engine.state.objects[&original].zone,
        Zone::Exile,
        "departed spell must not resume its return phase"
    );
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn scrap_mastery_copy_entrant_owner_departure_skips_only_that_entrant() {
    let mut engine = setup();
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let departed = inject_graveyard_card(&mut engine, 1, "phyrexian_metamorph");
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    let spell = cast(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );
    engine.apply_command(9, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&departed));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&original].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_timestamp_owner_departure_retains_surviving_order() {
    let mut engine = setup();
    for seat in [1, 2] {
        inject_graveyard_card(&mut engine, seat, "sol_ring");
        inject_graveyard_card(&mut engine, seat, "mind_stone");
    }
    let spell = cast(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::SimultaneousEntryOrder
    );
    engine.apply_command(9, &concede()).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        27
    );
    order_all(&mut engine);
    assert_eq!(engine.state.players[2].battlefield.len(), 2);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_copy_source_departure_refreshes_without_removing_entrant() {
    let mut engine = setup();
    let source = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let entrant = inject_graveyard_card(&mut engine, 2, "phyrexian_metamorph");
    let spell = cast(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .contains(&source));
    engine.apply_command(9, &concede()).unwrap();
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_none_or(|pending| !pending.presentation.candidates.contains(&source)));
    if engine.state.pending_resolution.is_some() {
        engine
            .apply_command(27, &submit_resolution_choice(vec![]))
            .unwrap();
    }
    assert_eq!(
        engine.state.objects[&entrant].zone,
        Zone::Graveyard,
        "declined Metamorph enters as a 0/0 then dies"
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

fn watch_return(engine: &mut GameEngine, watched: u32, returned: u32) {
    use tricerules_core::state::{
        ActiveEventObserver, EventObserverMatcher, EventObserverPayload, TriggerObjectRef,
    };
    let watched_ref = TriggerObjectRef {
        object_id: watched,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&watched)
            .copied()
            .unwrap_or(0),
        controller_at_event: engine.state.objects[&watched].controller,
    };
    let returned_ref = TriggerObjectRef {
        object_id: returned,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&returned)
            .copied()
            .unwrap_or(0),
        controller_at_event: engine.state.objects[&returned].controller,
    };
    engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched: Some(watched_ref),
            matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
            payload: EventObserverPayload::ReturnExiledObject {
                exiled: returned_ref,
            },
        });
}

fn exiled_fixture(engine: &mut GameEngine, seat: usize, card: &str) -> u32 {
    let oid = inject_graveyard_card(engine, seat, card);
    engine.state.players[seat].graveyard.retain(|id| *id != oid);
    engine.state.players[seat].exile.push(oid);
    engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Exile;
    oid
}

#[test]
fn scrap_mastery_caster_departure_detaches_owed_aura_and_queued_returns() {
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    let recipient = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let watched = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let aura = exiled_fixture(&mut engine, 1, "pacifism");
    let queued = exiled_fixture(&mut engine, 2, "short_sword");
    watch_return(&mut engine, watched, aura);
    watch_return(&mut engine, watched, queued);
    let spell = cast(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::AuraPermanent
    );
    engine.apply_command(4, &concede()).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("owed Aura return remains selectable")
            .deciding_player,
        9
    );
    engine
        .apply_command(9, &submit_resolution_choice(vec![recipient]))
        .unwrap();
    order_all(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(recipient))
    );
    assert_eq!(engine.state.objects[&queued].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&original].zone, Zone::Exile);
    assert!(!engine.state.objects.contains_key(&spell));
}

#[test]
fn scrap_mastery_last_order_departure_flushes_surviving_committed_departure_trigger() {
    use tricerules_cards::{CastTriggerPlayer, TriggerCondition};
    let mut engine = setup();
    let observer = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let mut ability = tricerules_cards::registry::global()
        .get("ajanis_pridemate")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WheneverPermanentLeavesBattlefield {
        controller: CastTriggerPlayer::AnyPlayer,
        filter: Default::default(),
        destination: Default::default(),
        cardinality: tricerules_cards::primitives::ZoneEventCardinality::OneOrMore,
    };
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(observer),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    cast(&mut engine);
    assert!(engine.state.stack.is_empty());
    engine.apply_command(9, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&original].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.source_permanent_id == Some(observer))
            .count(),
        1,
        "committed history remains observable once and is placed in the concession command"
    );
}

#[test]
fn scrap_mastery_unrelated_concession_does_not_check_life_during_entry_choice() {
    let mut engine = setup();
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 1, "phyrexian_metamorph");
    cast(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );
    // Model the life value left by an earlier payment in the still-incomplete resolution.
    // The pause and concession are actual commands; no SBA is permitted at this boundary.
    engine.state.players[0].life = 0;
    engine.apply_command(27, &concede()).unwrap();
    assert!(
        !engine.state.players[0].has_lost,
        "life-based loss waits until the entry transaction completes"
    );
    assert!(!engine.state.is_terminal());
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );
    assert!(engine.state.objects.contains_key(&source));
}

#[test]
fn scrap_mastery_caster_departure_detaches_owed_copy_entry_and_queued_returns() {
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    let source = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let watched = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let copy = exiled_fixture(&mut engine, 1, "phyrexian_metamorph");
    let queued = exiled_fixture(&mut engine, 2, "short_sword");
    watch_return(&mut engine, watched, copy);
    watch_return(&mut engine, watched, queued);
    let spell = cast(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::CopySource
    );
    engine.apply_command(4, &concede()).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );
    engine
        .apply_command(9, &submit_resolution_choice(vec![source]))
        .unwrap();
    order_all(&mut engine);
    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(engine.characteristics(copy).unwrap().power, Some(2));
    assert_eq!(engine.state.objects[&queued].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&original].zone, Zone::Exile);
    assert!(!engine.state.objects.contains_key(&spell));
}

#[test]
fn scrap_mastery_owed_aura_owner_departure_skips_it_and_resumes_survivors() {
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let watched = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let aura = exiled_fixture(&mut engine, 1, "pacifism");
    let queued = exiled_fixture(&mut engine, 2, "short_sword");
    watch_return(&mut engine, watched, aura);
    watch_return(&mut engine, watched, queued);
    let spell = cast(&mut engine);
    engine.apply_command(9, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&aura));
    order_all(&mut engine);
    assert_eq!(
        engine.state.objects[&queued].zone,
        Zone::Battlefield,
        "the already committed sacrifice must not repeat after the independent return"
    );
    assert_eq!(engine.state.objects[&original].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_detached_synthetic_entry_owner_departure_preserves_independent_queue() {
    let deck = deck_with("mountain", &["scrap_mastery"]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        509_104,
        &[4, 9, 27, 42],
        20,
        Some(vec![deck; 4]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "scrap_mastery");
    let original = inject_graveyard_card(&mut engine, 2, "sol_ring");
    inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let watched = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let copy = exiled_fixture(&mut engine, 1, "phyrexian_metamorph");
    let queued = exiled_fixture(&mut engine, 2, "short_sword");
    watch_return(&mut engine, watched, copy);
    watch_return(&mut engine, watched, queued);
    let spell = cast(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::CopySource
    );
    engine.apply_command(4, &concede()).unwrap();
    engine.apply_command(9, &concede()).unwrap();
    assert!(!engine.state.is_terminal());
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.objects.contains_key(&copy));
    assert!(!engine.state.objects.contains_key(&spell));
    assert_eq!(engine.state.objects[&queued].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&original].zone,
        Zone::Exile,
        "the cancelled outer tail never returns"
    );
}

#[test]
fn scrap_mastery_concession_accepts_but_abandons_stale_surviving_sacrifice_arrival() {
    let mut engine = setup();
    inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    let survivor = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
    cast(&mut engine);
    *engine
        .state
        .zone_change_generation
        .get_mut(&survivor)
        .unwrap() += 1;
    let generation = engine.state.zone_change_generation[&survivor];
    let batch = engine.apply_command(9, &concede()).unwrap();
    assert_eq!(engine.state.objects[&survivor].zone, Zone::Graveyard);
    assert_eq!(engine.state.zone_change_generation[&survivor], generation);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackResolved(_))))
            .count(),
        1
    );
}

#[test]
fn scrap_mastery_concession_accepts_but_abandons_stale_surviving_timestamp_entry() {
    let mut engine = setup();
    inject_graveyard_card(&mut engine, 1, "sol_ring");
    inject_graveyard_card(&mut engine, 1, "mind_stone");
    let survivor = inject_graveyard_card(&mut engine, 2, "sol_ring");
    cast(&mut engine);
    *engine
        .state
        .zone_change_generation
        .get_mut(&survivor)
        .unwrap() += 1;
    let generation = engine.state.zone_change_generation[&survivor];
    let batch = engine.apply_command(9, &concede()).unwrap();
    assert_eq!(engine.state.objects[&survivor].zone, Zone::Exile);
    assert_eq!(engine.state.zone_change_generation[&survivor], generation);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::StackResolved(_))))
            .count(),
        1
    );
}

#[test]
fn scrap_mastery_excludes_replacement_exile_and_sacrifices_stolen_artifact_lands_creatures_and_tokens(
) {
    use tricerules_core::state::ActiveDeathReplacement;
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 1, "darksteel_myr");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "darksteel_citadel");
    let replaced = inject_creature_on_battlefield(&mut engine, 2, "darksteel_myr");
    let stolen = inject_permanent_on_battlefield(&mut engine, 2, "mind_stone");
    engine.state.players[2]
        .battlefield
        .retain(|oid| *oid != stolen);
    engine.state.players[1].battlefield.push(stolen);
    engine.state.objects.get_mut(&stolen).unwrap().controller = 9;
    engine
        .state
        .death_replacement_effects
        .push(ActiveDeathReplacement {
            object_id: replaced,
            zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&replaced)
                .copied()
                .unwrap_or(0),
        });
    let token = inject_creature_on_battlefield(&mut engine, 2, "soldier_w_1_1");
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(token),
        kind: ContinuousEffectKind::Layer4AddTypes(
            tricerules_cards::primitives::TypeLineAddition {
                card_types: vec![tricerules_cards::PermanentTypeFilter::Artifact],
                ..Default::default()
            },
        ),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    cast(&mut engine);
    order_all(&mut engine);
    assert_eq!(engine.state.objects[&original].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&original].controller, 9);
    assert_eq!(
        engine.state.objects[&replaced].zone,
        Zone::Exile,
        "replacement-exiled sacrifices are outside the original cohort"
    );
    for oid in [land, stolen] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }
    assert!(engine.state.players[2].graveyard.contains(&stolen));
    assert!(!engine.state.players[1].graveyard.contains(&stolen));
    assert!(
        !engine.state.objects.contains_key(&token),
        "sacrificed tokens cease at completed-resolution SBA"
    );
}

#[test]
fn scrap_mastery_freezes_current_derived_artifact_membership_before_source_departure() {
    let mut engine = setup();
    let source = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let gained = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: Some(source),
        trigger_grant_origin: None,
        affected: AffectedScope::Single(gained),
        kind: ContinuousEffectKind::Layer4AddTypes(
            tricerules_cards::primitives::TypeLineAddition {
                card_types: vec![tricerules_cards::PermanentTypeFilter::Artifact],
                ..Default::default()
            },
        ),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    cast(&mut engine);
    order_all(&mut engine);
    for oid in [source, gained] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }
}

#[test]
fn scrap_mastery_present_empty_original_cohort_completes_without_a_choice() {
    let mut engine = setup();
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let nonartifact = inject_graveyard_card(&mut engine, 2, "grizzly_bears");
    let spell = cast(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&nonartifact].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_paid_cast_choices_and_concession_replay_encoded_commands() {
    use prost::Message;
    fn fresh() -> GameEngine {
        let mut engine = setup();
        for seat in 0..3 {
            inject_graveyard_card(&mut engine, seat, "sol_ring");
            inject_permanent_on_battlefield(&mut engine, seat, "short_sword");
            inject_permanent_on_battlefield(&mut engine, seat, "mind_stone");
        }
        engine.state.players[0].mana_pool.colorless = 3;
        engine.state.players[0].mana_pool.red = 2;
        engine
    }
    let mut engine = fresh();
    let slot = hand_index_for_card(&engine, 0, "scrap_mastery");
    let mut commands = vec![(4, cast_spell(slot, vec![]))];
    let mut batches = vec![engine.apply_command(4, &commands[0].1).unwrap()];
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        let command = pass();
        batches.push(engine.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    let command = concede();
    batches.push(engine.apply_command(9, &command).unwrap());
    commands.push((9, command));
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        let actor = pending.deciding_player;
        let mut ids = pending.presentation.candidates.clone();
        ids.reverse();
        let command = submit_resolution_choice(ids);
        batches.push(engine.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    let mut replay = fresh();
    for ((actor, command), expected) in commands.into_iter().zip(batches) {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), expected);
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn scrap_mastery_actual_twincast_resolves_a_fresh_cohort_in_each_spell() {
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 1, "sol_ring");
    let sacrificed = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let existing = exiled_fixture(&mut engine, 2, "mind_stone");
    engine.state.players[0].mana_pool.colorless = 3;
    engine.state.players[0].mana_pool.red = 2;
    let slot = hand_index_for_card(&engine, 0, "scrap_mastery");
    let spell = engine.state.players[0].hand[slot];
    engine.apply_command(4, &cast_spell(slot, vec![])).unwrap();
    engine.apply_command(4, &pass()).unwrap();
    inject_card_into_hand(&mut engine, 1, "twincast");
    engine.state.players[1].mana_pool.blue = 2;
    let twin_slot = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_command(9, &cast_spell(twin_slot, target_object(spell)))
        .unwrap();
    for _ in 0..3 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    assert!(
        engine.state.pending_resolution.is_none(),
        "a nontargeted spell copy needs no retargeting choice"
    );
    assert!(engine.state.stack.last().unwrap().is_copy);
    for _ in 0..3 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    order_all(&mut engine);
    assert_eq!(engine.state.objects[&original].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&sacrificed].zone, Zone::Graveyard);
    for _ in 0..3 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    order_all(&mut engine);
    assert_eq!(engine.state.objects[&original].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&sacrificed].zone,
        Zone::Battlefield,
        "the original spell captures its own resolution-time graveyard"
    );
    assert_eq!(engine.state.objects[&existing].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_unexpected_missing_survivor_abandons_the_entire_proposed_return() {
    let mut engine = setup();
    let retained = inject_graveyard_card(&mut engine, 0, "sol_ring");
    inject_graveyard_card(&mut engine, 1, "sol_ring");
    inject_graveyard_card(&mut engine, 1, "mind_stone");
    let missing = inject_graveyard_card(&mut engine, 2, "sol_ring");
    let spell = cast(&mut engine);
    // Distinguish an unexpected missing surviving-owner object from CR800.4 owner removal.
    engine.state.objects.remove(&missing);
    engine.state.players[2].exile.retain(|oid| *oid != missing);
    engine.apply_command(9, &concede()).unwrap();
    assert_eq!(
        engine.state.objects[&retained].zone,
        Zone::Exile,
        "do not silently omit the stale member and commit other proposed entries"
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_stale_observer_entry_preserves_valid_owed_returns_and_cancels_tail() {
    let mut engine = setup();
    let original = inject_graveyard_card(&mut engine, 0, "sol_ring");
    let watched = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let stale = exiled_fixture(&mut engine, 0, "short_sword");
    let valid = [
        exiled_fixture(&mut engine, 1, "sol_ring"),
        exiled_fixture(&mut engine, 1, "short_sword"),
    ];
    for oid in [stale, valid[0], valid[1]] {
        watch_return(&mut engine, watched, oid);
    }
    let spell = cast(&mut engine);
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
        .entry(stale)
        .or_default() += 1;
    let generation = engine.state.zone_change_generation[&stale];
    let batch = engine.apply_command(27, &concede()).unwrap();
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::PriorityChanged(_)))));
    order_all(&mut engine);
    for oid in valid {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    }
    assert_eq!(engine.state.objects[&stale].zone, Zone::Exile);
    assert_eq!(engine.state.zone_change_generation[&stale], generation);
    assert_eq!(engine.state.objects[&original].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn scrap_mastery_future_timestamp_departure_preserves_already_chosen_order() {
    let mut engine = setup();
    let mut originals = Vec::new();
    for seat in 0..3 {
        originals.push([
            inject_graveyard_card(&mut engine, seat, "sol_ring"),
            inject_graveyard_card(&mut engine, seat, "mind_stone"),
        ]);
    }
    cast(&mut engine);
    engine
        .apply_command(
            4,
            &submit_resolution_choice(vec![originals[0][1], originals[0][0]]),
        )
        .unwrap();
    engine.apply_command(27, &concede()).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );
    let before = format!("{:?}", engine.state);
    for ids in [
        vec![originals[1][0]; 2],
        vec![originals[0][0], originals[1][1]],
        vec![originals[1][0]],
    ] {
        assert!(engine
            .apply_command(9, &submit_resolution_choice(ids))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
    engine
        .apply_command(
            9,
            &submit_resolution_choice(vec![originals[1][1], originals[1][0]]),
        )
        .unwrap();
    let serialized = engine.diagnostic_snapshot().unwrap();
    let timestamp = |oid: u32| {
        serialized["state"]["battlefield_entry_timestamps"][oid.to_string()]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    for pair in &originals[..2] {
        assert!(timestamp(pair[1]) < timestamp(pair[0]));
    }
    assert!(timestamp(originals[0][0]) < timestamp(originals[1][1]));
}
