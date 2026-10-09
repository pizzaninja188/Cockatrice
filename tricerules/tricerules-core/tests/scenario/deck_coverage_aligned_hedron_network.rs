//! Actual-card coverage for Aligned Hedron Network's paired mass exile.

use super::helpers::*;
use tricerules_cards::{
    AbilityId, AbilityPresentation, ContinuousEffectKind, CounterKind, EffectDuration, Keyword,
    PermanentTypeFilter, TriggerCondition, TriggeredAbilityDef, TypeLineAddition,
};
use tricerules_core::state::{
    ActiveEventObserver, DelayedTriggerPayload, EventObserverMatcher, EventObserverPayload,
    TriggerObjectRef,
};
use tricerules_core::{AffectedScope, AttachmentRecipient, ContinuousEffect, GameEngine, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, RuledCommand};

fn setup() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_801,
        &[4, 9, 27],
        20,
        None,
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn creature_with_power(engine: &mut GameEngine, player: usize, power: u32) -> u32 {
    let object_id = inject_creature_on_battlefield(engine, player, "grizzly_bears");
    let extra_counters = power.saturating_sub(2);
    if extra_counters > 0 {
        engine
            .state
            .objects
            .get_mut(&object_id)
            .unwrap()
            .counters
            .insert(CounterKind::PlusOnePlusOne, extra_counters);
    }
    object_id
}

fn cast_network(engine: &mut GameEngine) -> u32 {
    let network = inject_card_into_hand(engine, 0, "aligned_hedron_network");
    engine.state.players[0].mana_pool.colorless = 4;
    let hand_index = hand_index_for_card(engine, 0, "aligned_hedron_network");
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast the actual Aligned Hedron Network");
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&network].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the ETB ability is on the stack"
    );
    assert!(engine.state.stack.last().unwrap().is_triggered);
    network
}

fn move_network_to_zone(
    engine: &mut GameEngine,
    player_id: i32,
    zone: DevZone,
) -> Vec<tricerules_proto::ruled::v1::RuledEvent> {
    engine.enable_dev_commands();
    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Aligned Hedron Network".into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the exact Network permanent to its owner's graveyard")
        .events
}

#[test]
fn aligned_hedron_network_exiles_current_power_five_creatures_and_returns_them_together() {
    let mut engine = setup();
    let own_creature = creature_with_power(&mut engine, 0, 5);
    let opponent_creature = creature_with_power(&mut engine, 1, 5);
    let stolen_creature = creature_with_power(&mut engine, 2, 5);
    let below_threshold = creature_with_power(&mut engine, 2, 4);

    engine.state.players[2]
        .battlefield
        .retain(|object_id| *object_id != stolen_creature);
    engine.state.players[1].battlefield.push(stolen_creature);
    engine
        .state
        .objects
        .get_mut(&stolen_creature)
        .unwrap()
        .controller = 9;
    engine
        .state
        .objects
        .get_mut(&stolen_creature)
        .unwrap()
        .base_controller = 9;
    assert_eq!(engine.state.objects[&stolen_creature].owner, 27);

    let network = cast_network(&mut engine);

    pass_priority_round(&mut engine);
    for object_id in [own_creature, opponent_creature, stolen_creature] {
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Exile);
        assert_eq!(engine.state.zone_change_generation[&object_id], 1);
    }
    assert_eq!(
        engine.state.objects[&below_threshold].zone,
        Zone::Battlefield
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    let returned = move_network_to_zone(&mut engine, 4, DevZone::Graveyard);
    for (object_id, owner) in [
        (own_creature, 4),
        (opponent_creature, 9),
        (stolen_creature, 27),
    ] {
        let object = &engine.state.objects[&object_id];
        assert_eq!(object.zone, Zone::Battlefield);
        assert_eq!(object.owner, owner);
        assert_eq!(object.controller, owner, "return under the owner's control");
        assert_eq!(engine.state.zone_change_generation[&object_id], 2);
        assert_eq!(object.counters.get(&CounterKind::PlusOnePlusOne), None);
        assert_eq!(engine.characteristics(object_id).unwrap().power, Some(2));
    }
    let moved_back: Vec<_> = returned
        .iter()
        .filter_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::PermanentMoved(moved))
                if [own_creature, opponent_creature, stolen_creature]
                    .contains(&moved.object_id)
                    && moved.destination
                        == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield
                            as i32 =>
            {
                Some(moved.object_id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        moved_back.len(),
        3,
        "the return is one complete public cohort"
    );
    assert!(engine.state.objects[&network].zone == Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&below_threshold].zone,
        Zone::Battlefield
    );
}

#[test]
fn aligned_hedron_network_etb_does_nothing_after_its_source_leaves() {
    let mut engine = setup();
    let creature = creature_with_power(&mut engine, 1, 5);
    let network = cast_network(&mut engine);

    move_network_to_zone(&mut engine, 4, DevZone::Graveyard);
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&network].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn aligned_hedron_network_checks_power_when_its_etb_trigger_resolves() {
    let mut engine = setup();
    let drops_below_threshold = creature_with_power(&mut engine, 0, 5);
    let rises_to_threshold = creature_with_power(&mut engine, 1, 4);
    let hexproof_creature = creature_with_power(&mut engine, 2, 5);
    let network = cast_network(&mut engine);

    engine
        .state
        .objects
        .get_mut(&drops_below_threshold)
        .unwrap()
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    engine
        .state
        .objects
        .get_mut(&rises_to_threshold)
        .unwrap()
        .counters
        .insert(CounterKind::PlusOnePlusOne, 3);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(hexproof_creature),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Hexproof),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    assert!(engine.effective_has_keyword(hexproof_creature, Keyword::Hexproof));
    pass_priority_round(&mut engine);

    assert_eq!(
        engine.state.objects[&drops_below_threshold].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&rises_to_threshold].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&hexproof_creature].zone, Zone::Exile);
    assert_eq!(engine.state.zone_change_generation[&rises_to_threshold], 1);
    assert_eq!(engine.state.stack.len(), 0, "the ETB trigger has resolved");
    let later_creature = creature_with_power(&mut engine, 1, 5);
    assert_eq!(
        engine.state.objects[&later_creature].zone,
        Zone::Battlefield
    );

    move_network_to_zone(&mut engine, 4, DevZone::Graveyard);
    assert_eq!(engine.state.objects[&network].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&drops_below_threshold].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&rises_to_threshold].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&hexproof_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&later_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.zone_change_generation[&rises_to_threshold], 2);
    assert_eq!(
        engine.state.objects[&rises_to_threshold]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        None
    );
}

#[test]
fn aligned_hedron_network_old_etb_does_not_apply_to_a_later_source_generation() {
    let mut engine = setup();
    let first_creature = creature_with_power(&mut engine, 1, 5);
    let network = cast_network(&mut engine);
    let first_generation = engine.state.zone_change_generation[&network];

    move_network_to_zone(&mut engine, 4, DevZone::Graveyard);
    move_network_to_zone(&mut engine, 4, DevZone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&network],
        first_generation + 2
    );
    assert_eq!(
        engine.state.stack.len(),
        2,
        "both entry triggers are retained"
    );

    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&first_creature].zone, Zone::Exile);
    let later_creature = creature_with_power(&mut engine, 2, 5);
    pass_priority_round(&mut engine);

    assert_eq!(
        engine.state.objects[&later_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&first_creature].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn aligned_hedron_network_self_exile_returns_once_and_does_not_recreate_tokens() {
    let mut engine = setup();
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    let other_creature = creature_with_power(&mut engine, 2, 5);
    assert!(engine.state.objects[&token].is_token());
    engine
        .state
        .objects
        .get_mut(&token)
        .unwrap()
        .counters
        .insert(CounterKind::PlusOnePlusOne, 4);
    let network = cast_network(&mut engine);
    {
        let object = engine.state.objects.get_mut(&network).unwrap();
        object.power = Some(5);
        object.toughness = Some(5);
    }
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(network),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..Default::default()
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    assert!(engine.characteristics(network).unwrap().is_creature());
    assert_eq!(engine.state.players[0].battlefield.first(), Some(&network));

    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&network].zone, Zone::Battlefield);
    assert_eq!(engine.state.zone_change_generation[&network], 4);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the returned Network triggers once"
    );
    assert!(engine.state.stack.last().unwrap().is_triggered);
    assert_eq!(
        engine.state.objects[&other_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.zone_change_generation[&other_creature], 2);
    assert!(!engine.state.objects.contains_key(&token));
    assert!(!engine.state.players[1].battlefield.contains(&token));
}

#[test]
fn aligned_hedron_network_owner_concession_returns_survivors_before_publication() {
    let mut engine = setup();
    let departing_owner_creature = creature_with_power(&mut engine, 0, 5);
    let surviving_creature = creature_with_power(&mut engine, 1, 5);
    let network = cast_network(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&surviving_creature].zone, Zone::Exile);
    let network_ref = TriggerObjectRef {
        object_id: network,
        zone_change_generation: engine.state.zone_change_generation[&network],
        controller_at_event: 4,
    };
    engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched: Some(network_ref),
            matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
            payload: EventObserverPayload::StageDelayedTrigger(Box::new(DelayedTriggerPayload {
                source: network_ref,
                controller: 9,
                affected_player: None,
                card_id: "aligned_hedron_network".into(),
                card_name: "Aligned Hedron Network".into(),
                source_face_index: 0,
                presentation: Some(tricerules_proto::ruled::v1::PresentationRef {
                    card_id: "aligned_hedron_network".into(),
                    face_id: "front".into(),
                    fallback_text: "At the beginning of the next end step, do nothing.".into(),
                    ..Default::default()
                }),
                ability: TriggeredAbilityDef {
                    ability_id: AbilityId::new("unrelated_departure_probe")
                        .expect("valid test ability id"),
                    presentation: AbilityPresentation::Fallback,
                    trigger: TriggerCondition::AtBeginningOfNextEndStep,
                    effect: vec![],
                    modal: None,
                    targeting: None,
                    may: false,
                    intervening_if: None,
                    max_triggers_per_turn: None,
                    triggers_only_once: false,
                },
            })),
        });

    let concession = engine.apply_command(4, &concede()).expect("owner concedes");

    assert!(!engine.state.objects.contains_key(&departing_owner_creature));
    assert_eq!(
        engine.state.objects[&surviving_creature].zone,
        Zone::Battlefield
    );
    assert!(!engine.state.objects.contains_key(&network));
    assert!(
        engine.state.pending_triggers.is_empty()
            && engine.state.staged_trigger_groups.is_empty()
            && engine.state.stack.is_empty(),
        "the departing Network's unrelated delayed observer is not staged"
    );
    let return_index = concession
        .events
        .iter()
        .position(|event| matches!(&event.ev, Some(Ev::PermanentMoved(moved))
            if moved.object_id == surviving_creature
                && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32))
        .expect("surviving owner's card returns in the concession batch");
    let zone_view_index = concession
        .events
        .iter()
        .position(|event| matches!(event.ev, Some(Ev::ZoneView(_))))
        .expect("concession publishes a zone view");
    assert!(return_index < zone_view_index);
    if let Some(priority_index) = concession
        .events
        .iter()
        .position(|event| matches!(event.ev, Some(Ev::PriorityChanged(_))))
    {
        assert!(return_index < priority_index);
    }
}

#[test]
fn aligned_hedron_network_owner_concession_preserves_returning_aura_choice() {
    let mut engine = setup();
    let bearer = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 2, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(bearer));
    {
        let object = engine.state.objects.get_mut(&aura).unwrap();
        object.power = Some(5);
        object.toughness = Some(5);
    }
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(aura),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..Default::default()
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    let network = cast_network(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);

    let concession = engine.apply_command(4, &concede()).expect("owner concedes");

    assert!(!engine.state.objects.contains_key(&network));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura return waits for its owner to choose a creature");
    assert_eq!(pending.deciding_player, 27);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&bearer));
    assert!(concession
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::ResolutionChoiceRequired(_)))));
    assert!(!concession
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::PriorityChanged(_)))));
}
