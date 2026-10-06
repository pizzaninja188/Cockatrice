//! All Is Dust: one simultaneous sacrifice with owner-chosen graveyard order.
use super::helpers::*;
use tricerules_cards::{CardRegistry, Color, ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::state::ActiveDeathReplacement;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::ChoiceKind;

fn setup() -> GameEngine {
    let deck = deck_with("forest", &["all_is_dust"]);
    let mut engine =
        GameEngine::new(26_100_101, &[0, 1, 2], 20, Some(vec![deck; 3]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "all_is_dust");
    engine
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let mut batch = Default::default();
    for _ in 0..engine.state.players.len() {
        batch = engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    batch
}

fn cast(engine: &mut GameEngine) -> RuledEventBatch {
    let actor = engine.state.players[0].id;
    engine.state.players[0].mana_pool.colorless = 7;
    let slot = hand_index_for_card(engine, 0, "all_is_dust");
    engine
        .apply_command(actor, &cast_spell(slot, vec![]))
        .unwrap();
    resolve_one(engine)
}

fn order_all(engine: &mut GameEngine) {
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        let player = pending.deciding_player;
        let mut choices = pending.presentation.candidates.clone();
        choices.reverse();
        engine
            .apply_command(player, &submit_resolution_choice(choices))
            .unwrap();
    }
}

fn modify(engine: &mut GameEngine, oid: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn reject(engine: &mut GameEngine, actor: i32, ids: Vec<u32>) {
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(actor, &submit_resolution_choice(ids))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before, "rejection is atomic");
}

#[test]
fn all_is_dust_paid_cast_sacrifices_once_and_orders_each_owners_arrivals() {
    let mut engine = setup();
    let mut victims = Vec::new();
    for seat in 0..3 {
        victims.push(vec![
            inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears"),
            inject_creature_on_battlefield(&mut engine, seat, "hill_giant"),
        ]);
    }
    let land = inject_creature_on_battlefield(&mut engine, 0, "forest");
    engine.state.players[0].mana_pool.colorless = 7;
    let slot = hand_index_for_card(&engine, 0, "all_is_dust");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let first = resolve_one(&mut engine);
    for &oid in victims.iter().flatten() {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    let choice = find_resolution_choice(&first).expect("first owner's graveyard order");
    assert_eq!(choice.choice_kind(), ChoiceKind::GraveyardCards);
    assert!(choice.ordered);
    for (seat, cohort) in victims.iter().enumerate() {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, seat as i32);
        assert_eq!(pending.presentation.min, 2);
        assert_eq!(pending.presentation.max, 2);
        let order = vec![cohort[1], cohort[0]];
        engine
            .apply_command(seat as i32, &submit_resolution_choice(order.clone()))
            .unwrap();
        let graveyard = &engine.state.players[seat].graveyard;
        assert_eq!(
            graveyard
                .iter()
                .copied()
                .filter(|oid| cohort.contains(oid))
                .collect::<Vec<_>>(),
            order
        );
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn all_is_dust_choice_rejections_preserve_pending_and_older_graveyard_order() {
    let mut engine = setup();
    let old = inject_graveyard_card(&mut engine, 0, "island");
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    cast(&mut engine);
    assert!(serde_json::to_string(&engine.state.pending_resolution).is_ok());
    reject(&mut engine, 1, vec![first, second]);
    reject(&mut engine, 0, vec![first]);
    reject(&mut engine, 0, vec![first, first]);
    reject(&mut engine, 0, vec![first, old]);
    let generation = engine.state.zone_change_generation[&second];
    engine
        .state
        .zone_change_generation
        .insert(second, generation + 2);
    reject(&mut engine, 0, vec![second, first]);
    engine
        .state
        .zone_change_generation
        .insert(second, generation);
    engine
        .apply_command(0, &submit_resolution_choice(vec![second, first]))
        .unwrap();
    assert_eq!(
        &engine.state.players[0].graveyard[..3],
        &[old, second, first]
    );
    assert_eq!(engine.state.zone_change_generation[&second], generation);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn all_is_dust_uses_resolution_colors_deduplicates_multicolor_and_excludes_tokens_from_order() {
    let mut engine = setup();
    let gained = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let lost = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let multi = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let token = inject_creature_on_battlefield(&mut engine, 2, "soldier_w_1_1");
    let artifact = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
    engine.state.players[0].mana_pool.colorless = 7;
    let slot = hand_index_for_card(&engine, 0, "all_is_dust");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    modify(
        &mut engine,
        gained,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
    );
    modify(
        &mut engine,
        lost,
        ContinuousEffectKind::Layer5SetColors(vec![]),
    );
    modify(
        &mut engine,
        multi,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Green, Color::Red]),
    );
    let batch = resolve_one(&mut engine);
    assert!(
        engine.state.pending_resolution.is_none(),
        "one card per owner needs no ordering"
    );
    for oid in [gained, multi] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|ev| matches!(&ev.ev, Some(Ev::PermanentMoved(m)) if m.object_id == oid))
                .count(),
            1
        );
    }
    for oid in [lost, artifact] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    }
    assert!(!engine.state.players[2].battlefield.contains(&token));
    assert!(!engine.state.players[2].graveyard.contains(&token));
}

#[test]
fn all_is_dust_sacrifice_bypasses_indestructible_regeneration_and_targeting_protection() {
    let mut engine = setup();
    let victim = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    for keyword in [Keyword::Indestructible, Keyword::Hexproof, Keyword::Shroud] {
        modify(
            &mut engine,
            victim,
            ContinuousEffectKind::Layer6AddKeyword(keyword),
        );
    }
    engine
        .state
        .objects
        .get_mut(&victim)
        .unwrap()
        .regeneration_shields = 1;
    cast(&mut engine);
    assert_eq!(engine.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    order_all(&mut engine);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn all_is_dust_nonconsecutive_players_use_owner_not_controller_for_apnap_order() {
    let deck = deck_with("forest", &["all_is_dust"]);
    let mut engine =
        GameEngine::new(26_100_102, &[10, 30, 70], 20, Some(vec![deck; 3]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "all_is_dust");
    let stolen = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let sibling = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    let active = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let active_second = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    modify(
        &mut engine,
        stolen,
        ContinuousEffectKind::Layer2Control {
            controller: tricerules_cards::ControllerReference::Fixed(30),
        },
    );
    let batch = cast(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        10
    );
    engine
        .apply_command(10, &submit_resolution_choice(vec![active_second, active]))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        70
    );
    assert!(engine.state.players[1].graveyard.is_empty());
    engine
        .apply_command(70, &submit_resolution_choice(vec![sibling, stolen]))
        .unwrap();
    assert_eq!(engine.state.players[2].graveyard, vec![sibling, stolen]);
    assert!(batch.events.iter().any(
        |ev| matches!(&ev.ev, Some(Ev::Log(log)) if log.text == "P30 sacrifices Grizzly Bears.")
    ));
    assert!(batch.events.iter().any(|ev| matches!(&ev.ev, Some(Ev::PermanentMoved(m)) if m.object_id == stolen && m.owner_player_id == 70 && m.controller_player_id == 70)));
}

fn grant_observer(
    engine: &mut GameEngine,
    source: u32,
    trigger: tricerules_cards::TriggerCondition,
) {
    let mut ability = CardRegistry::global()
        .get("ajanis_pridemate")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = trigger;
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn all_is_dust_departing_observers_collect_once_after_all_owner_choices() {
    use tricerules_cards::{CastTriggerPlayer, TriggerCondition};
    let mut engine = setup();
    for seat in 0..3 {
        let observer = inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears");
        inject_creature_on_battlefield(&mut engine, seat, "hill_giant");
        grant_observer(
            &mut engine,
            observer,
            TriggerCondition::WheneverPermanentLeavesBattlefield {
                controller: CastTriggerPlayer::Controller,
                filter: Default::default(),
                destination: Default::default(),
                cardinality: tricerules_cards::primitives::ZoneEventCardinality::OneOrMore,
            },
        );
    }
    cast(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "triggers stay parked through owner ordering"
    );
    order_all(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        3,
        "one event per departing observer"
    );
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .map(|item| item.controller)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn all_is_dust_replacement_exile_still_observes_sacrifice_without_death_or_ordering() {
    use tricerules_cards::{CastTriggerPlayer, TriggerCondition};
    let mut engine = setup();
    let victim = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_observer(
        &mut engine,
        victim,
        TriggerCondition::WheneverPlayerSacrificesPermanent {
            player: CastTriggerPlayer::Controller,
            filter: Default::default(),
        },
    );
    grant_observer(&mut engine, victim, TriggerCondition::WhenSelfDies);
    engine
        .state
        .death_replacement_effects
        .push(ActiveDeathReplacement {
            object_id: victim,
            zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&victim)
                .copied()
                .unwrap_or(0),
        });
    let batch = cast(&mut engine);
    assert_eq!(engine.state.objects[&victim].zone, Zone::Exile);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.stack.len(),
        1,
        "sacrifice trigger fires but death trigger does not"
    );
    assert!(batch.events.iter().any(|ev| matches!(&ev.ev, Some(Ev::PermanentMoved(m)) if m.object_id == victim && m.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Exile as i32)));
}

#[test]
fn all_is_dust_empty_colored_cohort_finishes_without_a_choice() {
    let mut engine = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    cast(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn all_is_dust_freezes_membership_before_a_color_granting_source_departs() {
    let mut engine = setup();
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let first = inject_creature_on_battlefield(&mut engine, 0, "darksteel_myr");
    let second = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(source),
        affected: AffectedScope::AllCreatures,
        kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    cast(&mut engine);
    for oid in [source, first, second] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }
    order_all(&mut engine);
}

#[test]
fn all_is_dust_mass_sacrifice_effect_tail_runs_once_after_the_last_owner_order() {
    use tricerules_cards::primitives::{TargetFilter, TargetKind};
    use tricerules_cards::{Amount, RelativePlayerSet, SpellEffectKind, TriggerCondition};
    let mut engine = setup();
    let source = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    for seat in 0..3 {
        inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears");
        inject_creature_on_battlefield(&mut engine, seat, "hill_giant");
    }
    grant_observer(
        &mut engine,
        source,
        TriggerCondition::WheneverSelfBecomesTapped,
    );
    let ContinuousEffectKind::GrantTriggeredAbility(ability) =
        &mut engine.state.continuous_effects.last_mut().unwrap().kind
    else {
        unreachable!()
    };
    ability.effect = vec![
        SpellEffectKind::SacrificeAll {
            players: RelativePlayerSet::All,
            filter: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
        },
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(3),
        },
    ];
    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .unwrap();
    resolve_one(&mut engine);
    for seat in 0..3 {
        assert_eq!(
            engine.state.players[0].life, 20,
            "tail waits for every owner"
        );
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, seat);
        let choices = pending.presentation.candidates.clone();
        engine
            .apply_command(seat, &submit_resolution_choice(choices))
            .unwrap();
    }
    assert_eq!(engine.state.players[0].life, 23);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn all_is_dust_paid_cast_and_owner_choices_replay_serialized_commands_identically() {
    use prost::Message;
    fn fresh() -> GameEngine {
        let mut engine = setup();
        for seat in 0..3 {
            inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears");
            inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears");
        }
        engine.state.players[0].mana_pool.colorless = 7;
        engine
    }
    let mut engine = fresh();
    let slot = hand_index_for_card(&engine, 0, "all_is_dust");
    let mut commands = vec![(0, cast_spell(slot, vec![]))];
    let mut expected = vec![engine.apply_command(0, &commands[0].1).unwrap()];
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        let command = pass();
        expected.push(engine.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        let actor = pending.deciding_player;
        let mut ids = pending.presentation.candidates.clone();
        ids.reverse();
        let command = submit_resolution_choice(ids);
        expected.push(engine.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    let mut replay = fresh();
    for ((actor, command), expected) in commands.into_iter().zip(expected) {
        let encoded = command.encode_to_vec();
        let decoded = RuledCommand::decode(encoded.as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), expected);
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

fn immediate_aura_fixture() -> (GameEngine, u32, u32, u32, u32, Vec<u32>) {
    use tricerules_core::state::{
        ActiveEventObserver, EventObserverMatcher, EventObserverPayload, TriggerObjectRef,
    };
    let mut engine = setup();
    let light = inject_permanent_on_battlefield(&mut engine, 0, "banishing_light");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let recipient = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    let other = vec![
        inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears"),
        inject_creature_on_battlefield(&mut engine, 1, "hill_giant"),
    ];
    let aura = inject_card_into_hand(&mut engine, 1, "pacifism");
    engine.state.players[1].hand.retain(|oid| *oid != aura);
    engine.state.players[1].exile.push(aura);
    engine.state.objects.get_mut(&aura).unwrap().zone = Zone::Exile;
    let watched = TriggerObjectRef {
        object_id: light,
        zone_change_generation: 0,
        controller_at_event: 0,
    };
    let exiled = TriggerObjectRef {
        object_id: aura,
        zone_change_generation: 0,
        controller_at_event: 1,
    };
    engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched: Some(watched),
            matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
            payload: EventObserverPayload::ReturnExiledObject { exiled },
        });
    (engine, light, bear, recipient, aura, other)
}

fn finish_aura_orders(
    engine: &mut GameEngine,
    light: u32,
    bear: u32,
    recipient: u32,
    aura: u32,
    other: &[u32],
) {
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::GraveyardCards,
        "owner ordering must survive the immediate Aura return"
    );
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    engine
        .apply_command(0, &submit_resolution_choice(vec![bear, light]))
        .unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::GraveyardCards);
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    engine
        .apply_command(1, &submit_resolution_choice(vec![other[1], other[0]]))
        .unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    engine
        .apply_command(1, &submit_resolution_choice(vec![recipient]))
        .unwrap();
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(recipient))
    );
    assert_eq!(&engine.state.players[0].graveyard[..2], &[bear, light]);
    assert_eq!(engine.state.players[1].graveyard, vec![other[1], other[0]]);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn all_is_dust_owner_order_survives_immediate_returning_aura_choice() {
    let (mut engine, light, bear, recipient, aura, other) = immediate_aura_fixture();
    grant_observer(
        &mut engine,
        light,
        tricerules_cards::TriggerCondition::WhenSelfLeavesBattlefield,
    );
    cast(&mut engine);
    assert!(engine.state.stack.is_empty());
    finish_aura_orders(&mut engine, light, bear, recipient, aura, &other);
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.source_permanent_id == Some(light))
            .count(),
        1,
        "departing observer collected exactly once"
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "All Is Dust has finished; only its observed departure trigger remains"
    );
}

#[test]
fn all_is_dust_immediate_aura_return_preserves_following_effect_tail() {
    use tricerules_cards::primitives::{TargetFilter, TargetKind};
    use tricerules_cards::{Amount, RelativePlayerSet, SpellEffectKind, TriggerCondition};
    let (mut engine, light, bear, recipient, aura, other) = immediate_aura_fixture();
    let source = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    grant_observer(
        &mut engine,
        source,
        TriggerCondition::WheneverSelfBecomesTapped,
    );
    let ContinuousEffectKind::GrantTriggeredAbility(ability) =
        &mut engine.state.continuous_effects.last_mut().unwrap().kind
    else {
        unreachable!()
    };
    ability.effect = vec![
        SpellEffectKind::SacrificeAll {
            players: RelativePlayerSet::All,
            filter: TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        is_color: Some(Color::Green),
                        ..Default::default()
                    },
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        is_color: Some(Color::White),
                        ..Default::default()
                    },
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        is_color: Some(Color::Red),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            },
        },
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(3),
        },
    ];
    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .unwrap();
    resolve_one(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
    engine
        .apply_command(0, &submit_resolution_choice(vec![bear, light]))
        .unwrap();
    assert_eq!(engine.state.players[0].life, 20);
    engine
        .apply_command(1, &submit_resolution_choice(vec![other[1], other[0]]))
        .unwrap();
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
    assert_eq!(
        engine.state.players[0].life, 20,
        "tail waits for the intervening Aura return choice"
    );
    engine
        .apply_command(1, &submit_resolution_choice(vec![recipient]))
        .unwrap();
    assert_eq!(engine.state.players[0].life, 23);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}
