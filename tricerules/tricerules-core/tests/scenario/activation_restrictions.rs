use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, AttachmentRecipient, ContinuousEffect, GameEngine};
use tricerules_proto::ruled::v1::{ruled_event::Ev, TargetRef};

fn activation_engine(seed: u64) -> GameEngine {
    anthem_engine(seed, "mountain")
}

fn target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }]
}

#[test]
fn celestial_enforcer_requires_a_flying_creature_controlled_by_its_controller() {
    let mut e = activation_engine(5401);
    let enforcer = inject_creature_on_battlefield(&mut e, 0, "celestial_enforcer");
    let target_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    assert_eq!(zone_view_ability_flags(&mut e, 0, enforcer), [false]);
    let err = e
        .apply_command(0, &activate_ability(enforcer, 0, target(target_creature)))
        .expect_err("the engine must reject activation while its condition is false");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));

    inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    assert_eq!(
        zone_view_ability_flags(&mut e, 0, enforcer),
        [false],
        "an opponent's flying creature does not satisfy 'you control'"
    );

    inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    assert_eq!(zone_view_ability_flags(&mut e, 0, enforcer), [true]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    e.apply_command(0, &activate_ability(enforcer, 0, target(target_creature)))
        .expect("the activation becomes legal after its condition is true");
    assert!(e.state.objects.get(&enforcer).expect("enforcer").tapped);
}

#[test]
fn goblin_bird_grabber_uses_the_same_condition_and_never_opens_target_selection() {
    let mut e = activation_engine(5402);
    let goblin = inject_creature_on_battlefield(&mut e, 0, "goblin_bird-grabber");
    assert_eq!(zone_view_ability_flags(&mut e, 0, goblin), [false]);

    inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    assert_eq!(zone_view_ability_flags(&mut e, 0, goblin), [true]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    e.apply_command(0, &activate_ability(goblin, 0, vec![]))
        .expect("source-bound keyword grants take no targets");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.characteristics(goblin)
            .expect("goblin characteristics")
            .has_keyword(Keyword::Flying),
        "the resolving ability grants flying to its physical source"
    );
}

#[test]
fn caged_zombie_tracks_committed_creature_deaths_for_ui_and_command_legality() {
    let mut e = activation_engine(5403);
    let zombie = inject_creature_on_battlefield(&mut e, 0, "caged_zombie");
    let gnomes = inject_creature_on_battlefield(&mut e, 0, "bottle_gnomes");
    assert_eq!(zone_view_ability_flags(&mut e, 0, zombie), [false]);

    e.apply_command(0, &activate_ability(gnomes, 0, vec![]))
        .expect("sacrifice Bottle Gnomes");
    assert_eq!(e.state.turn_history.current.creatures_died, 1);
    assert_eq!(zone_view_ability_flags(&mut e, 0, zombie), [true]);

    resolve_entire_stack_two_player(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    e.apply_command(0, &activate_ability(zombie, 0, vec![]))
        .expect("Caged Zombie is legal after a committed creature death");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 18);

    e.state.turn_history.finish_turn();
    assert_eq!(zone_view_ability_flags(&mut e, 0, zombie), [false]);
}

#[test]
fn attached_activation_prohibition_keeps_abilities_but_rejects_commands_and_follows_attachment() {
    let mut e = activation_engine(5405);
    let elves = inject_creature_on_battlefield(&mut e, 0, "llanowar_elves");
    let aura = inject_permanent_on_battlefield(&mut e, 0, "pacifism");
    e.state.objects.get_mut(&aura).expect("Aura").attached_to =
        Some(AttachmentRecipient::Object(elves));
    e.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(aura),
        affected: AffectedScope::AttachedTo(aura),
        kind: ContinuousEffectKind::ProhibitActivatedAbilities,
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: e.state.command_index,
    });

    assert_eq!(
        zone_view_ability_flags(&mut e, 0, elves),
        [false],
        "the ability remains listed but is not offered as activatable"
    );
    let public_batch = e.initial_response_batch();
    let zone_view = public_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("public zone view");
    let annotated = zone_view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == elves)
        .expect("enchanted creature remains in the battlefield view");
    assert!(annotated
        .rules_annotation_labels
        .contains(&"Activated abilities can't be activated".to_string()));
    let err = e
        .apply_command(0, &activate_ability(elves, 0, vec![]))
        .expect_err("the engine must reject a submitted prohibited activation");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert!(!e.state.objects[&elves].tapped);

    e.state.objects.get_mut(&aura).expect("Aura").attached_to = None;
    assert_eq!(zone_view_ability_flags(&mut e, 0, elves), [true]);
    e.apply_command(0, &activate_ability(elves, 0, vec![]))
        .expect("the activated ability works again after it is no longer enchanted");
    assert!(e.state.objects[&elves].tapped);
}

#[test]
fn attached_activation_prohibition_preserves_keywords_static_and_triggered_abilities() {
    let decks = Some(vec![
        deck_with("mountain", &["glimmerlight", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(5406, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut e);
    let equipment = move_ready_to_battlefield(&mut e, 0, "glimmerlight");
    resolve_entire_stack_two_player(&mut e);
    let aura = inject_permanent_on_battlefield(&mut e, 0, "confiscate");
    let equipped_creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.state.objects.get_mut(&aura).expect("Aura").attached_to =
        Some(AttachmentRecipient::Object(equipment));
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &activate_ability_for(&e, equipment, 0, target(equipped_creature)),
    )
    .expect("activate Glimmerlight's Equip ability");
    assert_eq!(e.state.stack.len(), 1);

    e.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(aura),
        affected: AffectedScope::AttachedTo(aura),
        kind: ContinuousEffectKind::ProhibitActivatedAbilities,
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: e.state.command_index,
    });
    e.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(equipment),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Flying),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: e.state.command_index,
    });
    let granted_ability = tricerules_cards::CardRegistry::global()
        .get("llanowar_elves")
        .expect("Llanowar Elves definition")
        .primary_face()
        .activated_abilities[0]
        .clone();
    e.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(equipment),
        kind: ContinuousEffectKind::GrantActivatedAbility(Box::new(granted_ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: e.state.command_index,
    });
    let granted_trigger = tricerules_cards::CardRegistry::global()
        .get("soul_warden")
        .expect("Soul Warden definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    e.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(equipment),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(granted_trigger)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: e.state.command_index,
    });

    assert_eq!(
        zone_view_ability_flags(&mut e, 0, equipment),
        [false, false]
    );
    assert!(e
        .characteristics(equipment)
        .is_some_and(|characteristics| characteristics.has_keyword(Keyword::Flying)));
    let err = e
        .apply_command(0, &activate_ability_for(&e, equipment, 1, vec![]))
        .expect_err("an activated ability granted after attachment is also prohibited");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.characteristics(equipped_creature)
            .and_then(|characteristics| characteristics.power),
        Some(3),
        "Glimmerlight's attached +1/+1 static ability applies after the prohibited state"
    );
    assert_eq!(
        e.state.stack.len(),
        0,
        "the already-activated Equip resolves"
    );

    move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, 21,
        "the granted trigger still resolves"
    );
}

#[test]
fn activation_history_changes_invalidate_the_battlefield_view_cache() {
    let mut e = activation_engine(5404);
    let zombie = inject_creature_on_battlefield(&mut e, 0, "caged_zombie");
    e.initial_response_batch();

    // Isolate the public history input from an accompanying battlefield move. The next ordinary
    // command must still publish fresh ability flags instead of claiming the battlefield view is
    // unchanged.
    e.state.turn_history.current.creatures_died = 1;
    let batch = e.apply_command(0, &pass()).expect("pass priority");
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("every settled batch publishes a zone view");
    assert!(!view.battlefields_unchanged);
    let zombie_view = view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == zombie)
        .expect("Caged Zombie in refreshed battlefield view");
    assert_eq!(
        zombie_view
            .activated_abilities
            .iter()
            .map(|ability| ability.activatable)
            .collect::<Vec<_>>(),
        [true]
    );
}
