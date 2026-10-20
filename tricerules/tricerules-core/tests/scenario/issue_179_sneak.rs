use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{CounterKind, PermanentTypeFilter};
use tricerules_cards::{ContinuousEffectKind, EffectDuration, TypeLineAddition};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection, dev_command, BlockPair, CastMethod, CastSpell, CostObjectRef, CostObjectRefs,
    CostSelection, DeclareAttackers, DevCommand, DevMoveCard, DevZone, HandActionKind,
    RuledCommand, TargetRefKind,
};

fn sneak_cast(engine: &GameEngine, slot: usize, attacker: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(hand_cast_source(slot)),
            cast_method: CastMethod::Sneak as i32,
            cost_selections: vec![CostSelection {
                cost_index: 0,
                selection: Some(cost_selection::Selection::BattlefieldObjects(
                    CostObjectRefs {
                        objects: vec![CostObjectRef {
                            object_id: attacker,
                            zone_change_generation: engine
                                .state
                                .zone_change_generation
                                .get(&attacker)
                                .copied()
                                .unwrap_or(0),
                        }],
                    },
                )),
            }],
            ..Default::default()
        })),
    }
}

fn setup_unblocked_attacker(seed: u64) -> (GameEngine, u32, usize) {
    setup_unblocked_sneak_card(seed, "foot_ninjas", "plains")
}

fn setup_unblocked_sneak_card(
    seed: u64,
    sneak_card: &str,
    basic_land: &str,
) -> (GameEngine, u32, usize) {
    let decks = Some(vec![
        deck_with(basic_land, &[sneak_card, "grizzly_bears"]),
        vec!["island".into(); 30],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_declare_attackers(&mut engine);
    ensure_card_in_hand(&mut engine, 0, sneak_card);
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();
    let slot = hand_index_for_card(&engine, 0, sneak_card);
    (engine, attacker, slot)
}

fn setup_shredder_with_target(seed: u64, target_card: &str) -> (GameEngine, u32, usize, u32) {
    let decks = Some(vec![
        deck_with("swamp", &["shredders_technique", "grizzly_bears"]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_declare_attackers(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "shredders_technique");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let target = inject_permanent_on_battlefield(&mut engine, 1, target_card);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();
    if engine.characteristics(target).unwrap().is_creature() {
        engine
            .apply_scenario_command(1, &declare_blockers(vec![]))
            .unwrap();
    }
    let slot = hand_index_for_card(&engine, 0, "shredders_technique");
    (engine, attacker, slot, target)
}

fn cast_shredder_at(engine: &mut GameEngine, attacker: u32, slot: usize, target: u32) {
    grant_pool(engine, 0);
    let mut command = sneak_cast(engine, slot, attacker);
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    cast.targets = target_object(target);
    cast.targets[0].kind = TargetRefKind::Permanent as i32;
    let result = engine.apply_scenario_command(0, &command);
    assert!(
        result.is_ok(),
        "Shredder announcement failed: {result:?}; priority={}; actions={:?}",
        engine.state.priority_player_id(),
        engine.initial_response_batch().legal_by_player[&0].hand_actions
    );
}

fn add_enchantment_type(engine: &mut GameEngine, object_id: u32) -> usize {
    let original_len = engine.state.continuous_effects.len();
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Enchantment],
            ..Default::default()
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    original_len
}

fn resolve_one_stack_item(engine: &mut GameEngine) {
    for _ in 0..engine.state.players.len() {
        engine
            .apply_scenario_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
}

fn set_object_controller(engine: &mut GameEngine, object_id: u32, controller: usize) {
    let new_controller = engine.state.players[controller].id;
    let object = engine.state.objects.get_mut(&object_id).unwrap();
    let old_controller = object.controller;
    if old_controller == new_controller {
        return;
    }
    object.controller = new_controller;
    for player in &mut engine.state.players {
        player.battlefield.retain(|oid| *oid != object_id);
    }
    engine.state.players[controller].battlefield.push(object_id);
}

#[test]
fn issue_179_sneak_is_published_after_blockers_with_a_generation_bound_return_cost() {
    let (mut engine, attacker, slot) = setup_unblocked_attacker(179_001);

    assert!(engine.state.combat.as_ref().unwrap().blockers_declared);
    let legal = engine.initial_response_batch();
    let action = legal.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| {
            action.hand_index == slot as u32
                && action.kind == HandActionKind::HandActionCastSpell as i32
                && action.cast_method == CastMethod::Sneak as i32
        })
        .expect("Sneak action after blockers");
    assert_eq!(action.cost, "{3}{W/B}");
    let choice = &action.cost_choices.as_ref().unwrap().choices[0];
    assert_eq!(choice.candidate_objects.len(), 1);
    assert_eq!(
        choice.candidate_objects[0]
            .object
            .as_ref()
            .unwrap()
            .object_id,
        attacker
    );
    assert_eq!(
        choice.candidate_objects[0]
            .object
            .as_ref()
            .unwrap()
            .zone_change_generation,
        engine
            .state
            .zone_change_generation
            .get(&attacker)
            .copied()
            .unwrap_or(0)
    );
}

#[test]
fn issue_179_sneak_returns_the_attacker_atomically_and_records_the_method() {
    let (mut engine, attacker, slot) = setup_unblocked_attacker(179_002);
    grant_pool(&mut engine, 0);
    let command = sneak_cast(&engine, slot, attacker);
    let batch = engine
        .apply_scenario_command(0, &command)
        .expect("pay Sneak cost");

    assert_eq!(engine.state.objects[&attacker].zone, Zone::Hand);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].cast_method.label(), Some("Sneak"));
    assert!(batch.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("returning Grizzly Bears"))
    }));
}

#[test]
fn issue_179_sneak_creature_enters_tapped_and_attacking_the_same_player() {
    let (mut engine, attacker, slot) = setup_unblocked_attacker(179_003);
    let assignment = engine.state.combat.as_ref().unwrap().attack_assignments[&attacker];
    let foot_ninjas = engine.state.players[0].hand[slot];
    grant_pool(&mut engine, 0);
    let command = sneak_cast(&engine, slot, attacker);
    engine.apply_scenario_command(0, &command).unwrap();
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&foot_ninjas].zone, Zone::Battlefield);
    assert!(engine.state.objects[&foot_ninjas].tapped);
    let combat = engine.state.combat.as_ref().unwrap();
    assert!(combat.attacking.contains(&foot_ninjas));
    assert_eq!(
        combat.attack_assignments[&foot_ninjas].defender,
        assignment.defender
    );
    assert_eq!(engine.state.players[0].life, 23, "ETB trigger still fires");
}

#[test]
fn issue_179_shredders_technique_loses_life_only_after_destroying_an_enchantment() {
    let decks = Some(vec![
        deck_with("swamp", &["shredders_technique", "grizzly_bears"]),
        deck_with("plains", &["glorious_anthem"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        179_004,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_declare_attackers(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "shredders_technique");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let enchantment = deploy_to_battlefield(&mut engine, 1, "glorious_anthem", false);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();

    let slot = hand_index_for_card(&engine, 0, "shredders_technique");
    grant_pool(&mut engine, 0);
    let mut command = sneak_cast(&engine, slot, attacker);
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    cast.targets = target_object(enchantment);
    cast.targets[0].kind = TargetRefKind::Permanent as i32;
    engine.apply_scenario_command(0, &command).unwrap();
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&enchantment].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[0].life, 18,
        "destroy result must feed the immediately following conditional"
    );
}

#[test]
fn issue_179_shredder_counts_enchantment_gained_before_destroy_and_loses_life_as_controller() {
    let (mut engine, attacker, slot, target) = setup_shredder_with_target(179_020, "grizzly_bears");
    set_object_controller(&mut engine, target, 0);
    assert_eq!(engine.state.objects[&target].owner, 1);
    assert_eq!(engine.state.objects[&target].controller, 0);
    cast_shredder_at(&mut engine, attacker, slot, target);
    add_enchantment_type(&mut engine, target);
    assert!(engine
        .characteristics(target)
        .unwrap()
        .has_type("Enchantment"));
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&target));
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_179_shredder_uses_destroy_time_types_after_enchantment_is_lost_before_resolution() {
    let (mut engine, attacker, slot, target) = setup_shredder_with_target(179_021, "grizzly_bears");
    let effect_count = add_enchantment_type(&mut engine, target);
    assert!(engine
        .characteristics(target)
        .unwrap()
        .has_type("Enchantment"));
    cast_shredder_at(&mut engine, attacker, slot, target);
    engine.state.continuous_effects.truncate(effect_count);
    assert!(engine.characteristics(target).unwrap().is_creature());
    assert!(!engine
        .characteristics(target)
        .unwrap()
        .has_type("Enchantment"));
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_179_shredder_counts_a_copied_enchantment_creature_and_keeps_target_owner() {
    let (mut engine, attacker, slot, target) = setup_shredder_with_target(179_022, "grizzly_bears");
    let face = tricerules_cards::registry::global()
        .get("appendage_amalgam")
        .unwrap()
        .primary_face()
        .clone();
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "appendage_amalgam".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    set_object_controller(&mut engine, target, 0);
    assert_eq!(engine.state.objects[&target].owner, 1);
    assert!(engine.characteristics(target).unwrap().is_creature());
    assert!(engine
        .characteristics(target)
        .unwrap()
        .has_type("Enchantment"));
    cast_shredder_at(&mut engine, attacker, slot, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&target));
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_179_shredder_counts_successful_destroy_replaced_with_exile() {
    let (mut engine, attacker, slot, target) = setup_shredder_with_target(179_023, "grizzly_bears");
    add_enchantment_type(&mut engine, target);
    engine
        .state
        .death_replacement_effects
        .push(tricerules_core::state::ActiveDeathReplacement {
            object_id: target,
            zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&target)
                .copied()
                .unwrap_or(0),
        });
    cast_shredder_at(&mut engine, attacker, slot, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_179_shredder_loses_no_life_when_destroy_is_prevented_by_indestructible_or_regeneration() {
    let (mut indestructible, attacker, slot, target) =
        setup_shredder_with_target(179_024, "darksteel_myr");
    add_enchantment_type(&mut indestructible, target);
    cast_shredder_at(&mut indestructible, attacker, slot, target);
    resolve_entire_stack_two_player(&mut indestructible);
    assert_eq!(
        indestructible.state.objects[&target].zone,
        Zone::Battlefield
    );
    assert_eq!(indestructible.state.players[0].life, 20);
    assert_eq!(indestructible.state.players[1].life, 20);

    let (mut regenerating, attacker, slot, target) =
        setup_shredder_with_target(179_025, "grizzly_bears");
    add_enchantment_type(&mut regenerating, target);
    regenerating
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .regeneration_shields = 1;
    cast_shredder_at(&mut regenerating, attacker, slot, target);
    resolve_entire_stack_two_player(&mut regenerating);
    assert_eq!(regenerating.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(regenerating.state.players[0].life, 20);
    assert_eq!(regenerating.state.players[1].life, 20);
}

#[test]
fn issue_179_shredder_does_not_resolve_for_a_stale_target_generation() {
    let (mut engine, attacker, slot, target) = setup_shredder_with_target(179_026, "grizzly_bears");
    add_enchantment_type(&mut engine, target);
    cast_shredder_at(&mut engine, attacker, slot, target);
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_179_shredder_keeps_its_result_while_destroy_waits_for_an_aura_return_choice() {
    use tricerules_core::state::{
        ActiveEventObserver, EventObserverMatcher, EventObserverPayload, TriggerObjectRef,
    };
    use tricerules_proto::ruled::v1::ChoiceKind;

    let (mut engine, attacker, slot, light) =
        setup_shredder_with_target(179_027, "banishing_light");
    let recipient = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = inject_card_into_hand(&mut engine, 1, "pacifism");
    engine.state.players[1].hand.retain(|oid| *oid != aura);
    engine.state.players[1].exile.push(aura);
    engine.state.objects.get_mut(&aura).unwrap().zone = Zone::Exile;
    let watched = TriggerObjectRef {
        object_id: light,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&light)
            .copied()
            .unwrap_or(0),
        controller_at_event: 1,
    };
    let exiled = TriggerObjectRef {
        object_id: aura,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&aura)
            .copied()
            .unwrap_or(0),
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

    cast_shredder_at(&mut engine, attacker, slot, light);
    resolve_one_stack_item(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(
        engine.state.players[0].life, 20,
        "the named-result tail is parked"
    );
    assert_eq!(engine.state.objects[&light].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![recipient]))
        .unwrap();
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(recipient))
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_179_sneak_is_not_available_before_blockers_or_for_a_blocked_attacker() {
    let decks = Some(vec![
        deck_with("plains", &["foot_ninjas", "grizzly_bears"]),
        deck_with("island", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        179_005,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_declare_attackers(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "foot_ninjas");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = put_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .unwrap();

    assert!(!engine.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .any(|action| action.cast_method == CastMethod::Sneak as i32));

    engine.apply_scenario_command(0, &pass()).unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();
    engine
        .apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .unwrap();

    assert!(!engine.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .any(|action| action.cast_method == CastMethod::Sneak as i32));
}

#[test]
fn issue_179_stale_attacker_generation_rejects_the_whole_payment() {
    let (mut engine, attacker, slot) = setup_unblocked_attacker(179_006);
    grant_pool(&mut engine, 0);
    let command = sneak_cast(&engine, slot, attacker);
    let mana_before = engine.state.players[0].mana_pool;
    let hand_before = engine.state.players[0].hand.clone();
    *engine
        .state
        .zone_change_generation
        .entry(attacker)
        .or_insert(0) += 1;

    assert!(engine.apply_scenario_command(0, &command).is_err());
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_179_sneak_can_cast_a_sorcery_in_the_post_blockers_window() {
    let (mut engine, attacker, slot) =
        setup_unblocked_sneak_card(179_007, "donatellos_technique", "island");
    let spell = engine.state.players[0].hand[slot];
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    grant_pool(&mut engine, 0);
    engine
        .apply_scenario_command(0, &sneak_cast(&engine, slot, attacker))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
}

#[test]
fn issue_179_sneak_entry_fires_etb_but_not_attack_triggers() {
    let (mut engine, attacker, slot) =
        setup_unblocked_sneak_card(179_008, "shredder,_unrelenting", "swamp");
    let shredder = engine.state.players[0].hand[slot];
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let command = sneak_cast(&engine, slot, attacker);
    engine.apply_scenario_command(0, &command).unwrap();
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&shredder].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "Sneak entry fires Shredder's ETB trigger, but CR 508 attack triggers do not fire"
    );
}

#[test]
fn issue_179_sneak_inherits_planeswalker_and_battle_defenders() {
    for (seed, defender_card, counter, battle) in [
        (179_009, "jace_beleren", CounterKind::Loyalty, false),
        (
            179_010,
            "invasion_of_ulgrotha_grandmother_ravi_sengir",
            CounterKind::Defense,
            true,
        ),
    ] {
        let decks = Some(vec![
            deck_with("plains", &["foot_ninjas"]),
            deck_with("island", &[defender_card]),
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            seed,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        advance_to_declare_attackers(&mut engine);
        ensure_card_in_hand(&mut engine, 0, "foot_ninjas");
        let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let defender = deploy_to_battlefield(&mut engine, 1, defender_card, false);
        engine
            .state
            .objects
            .get_mut(&defender)
            .unwrap()
            .set_counter(counter, 5);
        if battle {
            engine.state.battle_protectors.insert(defender, 1);
        }
        let assignment = engine.initial_response_batch().legal_by_player[&0]
            .legal_attack_assignments
            .iter()
            .find(|assignment| {
                assignment.attacker_object_id == attacker
                    && assignment.defender.as_ref().is_some_and(|target| {
                        target.kind == TargetRefKind::Permanent as i32
                            && target.object_id == defender
                    })
            })
            .cloned()
            .expect("permanent defender assignment");
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                        assignments: vec![assignment],
                    })),
                },
            )
            .unwrap();
        engine.apply_scenario_command(0, &pass()).unwrap();
        engine.apply_scenario_command(1, &pass()).unwrap();
        let paid_assignment = engine.state.combat.as_ref().unwrap().attack_assignments[&attacker];
        let slot = hand_index_for_card(&engine, 0, "foot_ninjas");
        let entrant = engine.state.players[0].hand[slot];
        grant_pool(&mut engine, 0);
        let command = sneak_cast(&engine, slot, attacker);
        engine.apply_scenario_command(0, &command).unwrap();
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(
            engine.state.combat.as_ref().unwrap().attack_assignments[&entrant].defender,
            paid_assignment.defender
        );
    }
}

#[test]
fn issue_179_sneak_enters_tapped_but_not_attacking_when_the_defender_is_stale() {
    let decks = Some(vec![
        deck_with("plains", &["foot_ninjas"]),
        deck_with("island", &["jace_beleren"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        179_011,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_declare_attackers(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "foot_ninjas");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let defender = deploy_to_battlefield(&mut engine, 1, "jace_beleren", false);
    engine
        .state
        .objects
        .get_mut(&defender)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    let assignment = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == attacker
                && assignment
                    .defender
                    .as_ref()
                    .is_some_and(|target| target.object_id == defender)
        })
        .cloned()
        .unwrap();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();
    let slot = hand_index_for_card(&engine, 0, "foot_ninjas");
    let entrant = engine.state.players[0].hand[slot];
    grant_pool(&mut engine, 0);
    let command = sneak_cast(&engine, slot, attacker);
    engine.apply_scenario_command(0, &command).unwrap();
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 1,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Jace Beleren".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    assert!(engine.state.objects[&entrant].tapped);
    assert!(!engine
        .state
        .combat
        .as_ref()
        .unwrap()
        .attacking
        .contains(&entrant));
}

#[test]
fn issue_179_spell_copy_keeps_the_sneak_choice_without_repaying_it() {
    let decks = Some(vec![
        deck_with("island", &["donatellos_technique", "grizzly_bears"]),
        deck_with("island", &["twincast"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        179_012,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_declare_attackers(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "donatellos_technique");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 1, "twincast");
    let attacker = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    let sneak_slot = hand_index_for_card(&engine, 0, "donatellos_technique");
    let command = sneak_cast(&engine, sneak_slot, attacker);
    engine.apply_scenario_command(0, &command).unwrap();
    let original = engine.state.stack[0].clone();

    engine.apply_scenario_command(0, &pass()).unwrap();
    let twincast_slot = hand_index_for_card(&engine, 1, "twincast");
    let mut stack_target = target_object(original.id);
    stack_target[0].kind = TargetRefKind::Stack as i32;
    engine
        .apply_scenario_command(1, &cast_spell(twincast_slot, stack_target))
        .unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();

    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy)
        .expect("Twincast copy");
    assert_eq!(copy.cast_method.label(), Some("Sneak"));
    assert_eq!(
        copy.returned_attacker_assignment,
        original.returned_attacker_assignment
    );
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Hand);
    assert_eq!(
        engine.state.players[0]
            .hand
            .iter()
            .filter(|&&object_id| object_id == attacker)
            .count(),
        1,
        "copying the spell never repays the return cost"
    );
}
