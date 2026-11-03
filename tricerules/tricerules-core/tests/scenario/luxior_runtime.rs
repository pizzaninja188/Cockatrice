//! Prepared-card Luxior runtime scenarios. These fixtures are isolated from production card data.
use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use std::sync::OnceLock;
use tricerules_cards::primitives::{CounterKind, ProtectionCardType, ProtectionQuality};
use tricerules_cards::CardRegistry;
use tricerules_cards::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
    TypeLineReplacement,
};
use tricerules_core::state::PhasedOutStatus;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::attachment_recipient::Recipient;

const LUXIOR: &str = "luxior_runtime_fixture";
const PLANESWALKER: &str = "luxior_planeswalker_fixture";
const CREATURE: &str = "luxior_creature_fixture";

const LUXIOR_RON: &str = r#"(
    id: "luxior_runtime_fixture", name: "Luxior Runtime Fixture",
    face_id: "luxior_runtime_fixture", mana_cost: "{1}",
    types: ["Artifact", "Equipment"], supertypes: ["Legendary"],
    static_abilities: [
        (ability_id: "static_01", presentation: Fallback,
            definition: AttachedModifier(
                count: Some(AttachedObjectCounterCount),
                power_per_match: 1, toughness_per_match: 1)),
        (ability_id: "static_02", presentation: Fallback,
            definition: AttachedModifier(
                add_types: (card_types: [Creature]), remove_planeswalker: true)),
    ],
    activated_abilities: [
        (ability_id: "activated_01", presentation: Fallback,
            costs: [Mana("{3}")],
            effect: [Equip(target: (kind: Creature, controller: You))]),
        (ability_id: "activated_02", presentation: Fallback,
            costs: [Mana("{1}")],
            effect: [EquipPlaneswalker(target:
                (kind: AnyPermanent, controller: You, permanent_types: [Planeswalker]))]),
    ],
)"#;

const PLANESWALKER_RON: &str = r#"(
    id: "luxior_planeswalker_fixture", name: "Luxior Planeswalker Fixture",
    face_id: "luxior_planeswalker_fixture", mana_cost: "{1}",
    types: ["Planeswalker", "Jace"], loyalty: Some(2),
    activated_abilities: [
        (ability_id: "activated_01", presentation: Fallback,
            costs: [Loyalty(1)], effect: [Draw(who: EachPlayer, count: 1)]),
        (ability_id: "activated_02", presentation: Fallback,
            costs: [Loyalty(-1)], effect: [Draw(who: EachPlayer, count: 1)]),
    ],
)"#;

const CREATURE_RON: &str = r#"(
    id: "luxior_creature_fixture", name: "Luxior Creature Fixture",
    face_id: "luxior_creature_fixture", mana_cost: "{1}",
    types: ["Creature", "Human"], power: 2, toughness: 2,
)"#;

fn luxior_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(&[LUXIOR_RON, PLANESWALKER_RON, CREATURE_RON], &[])
            .expect("isolated Luxior scenario fixtures are admitted")
    })
}

fn luxior_deck() -> Vec<String> {
    std::iter::repeat_n(LUXIOR.to_string(), 7)
        .chain(std::iter::repeat_n(PLANESWALKER.to_string(), 7))
        .chain(std::iter::repeat_n(CREATURE.to_string(), 6))
        .collect()
}

fn luxior_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        luxior_registry(),
        seed,
        &[0, 1],
        20,
        Some(vec![luxior_deck(), luxior_deck()]),
        true,
    )
    .expect("Luxior scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_fixture(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    ensure_in_hand(engine, player, card_id);
    give_mana(
        engine,
        player as i32,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let hand_slot = hand_index_for_card(engine, player, card_id);
    engine
        .apply_scenario_command(player as i32, &cast_spell(hand_slot, vec![]))
        .expect("cast isolated fixture through the engine command");
    resolve_entire_stack_two_player(engine);
    battlefield_object_for_card(engine, player, card_id)
}

fn target_ref(engine: &GameEngine, object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        expected_zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied(),
        ..Default::default()
    }
}

#[test]
fn luxior_equip_planeswalker_publishes_exact_target_and_attaches_through_commands() {
    let mut engine = luxior_engine(610_401);
    let luxior = cast_fixture(&mut engine, 0, LUXIOR);
    let planeswalker = cast_fixture(&mut engine, 0, PLANESWALKER);
    let creature = relocate_to_battlefield(&mut engine, 0, CREATURE, false);
    let opponent_planeswalker = relocate_to_battlefield(&mut engine, 1, PLANESWALKER, false);
    engine
        .state
        .objects
        .get_mut(&opponent_planeswalker)
        .expect("opponent planeswalker fixture")
        .set_counter(CounterKind::Loyalty, 2);

    let ability_key = (u64::from(luxior) << 32) | 1;
    let offered = engine.initial_response_batch();
    let target_group =
        &offered.legal_by_player[&0].valid_targets_by_ability[&ability_key].groups[0];
    assert_eq!(
        offered.legal_by_player[&0].valid_targets_by_ability[&ability_key]
            .groups
            .len(),
        1
    );
    assert!(target_group.valid_permanent_ids.contains(&planeswalker));
    assert!(!target_group.valid_permanent_ids.contains(&creature));
    assert!(!target_group
        .valid_permanent_ids
        .contains(&opponent_planeswalker));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let planeswalker_target = target_ref(&engine, planeswalker);
    apply_ability(&mut engine, 0, luxior, 1, vec![planeswalker_target])
        .expect("activate EquipPlaneswalker with the published target");
    assert_eq!(engine.state.stack.len(), 1, "activation is on the stack");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let mana_before_timing_rejection = engine.state.players[0].mana_pool.colorless;
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, luxior),
        [false, false]
    );
    let creature_target = target_ref(&engine, creature);
    assert!(
        apply_ability(&mut engine, 0, luxior, 0, vec![creature_target]).is_err(),
        "ordinary Equip remains sorcery-speed while another equip activation is on the stack"
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.players[0].mana_pool.colorless, mana_before_timing_rejection,
        "a rejected out-of-window Equip command does not pay its cost"
    );

    let first = engine.state.priority_player_id();
    let second = engine
        .state
        .players
        .iter()
        .find(|player| player.id != first)
        .expect("second player")
        .id;
    engine
        .apply_scenario_command(first, &pass())
        .expect("first player passes");
    let resolved = engine.apply_scenario_command(second, &pass());
    assert!(
        resolved.is_ok(),
        "EquipPlaneswalker must resolve instead of rejecting its typed effect: {:?}",
        resolved.err()
    );

    assert!(engine.state.stack.is_empty(), "activation resolves");
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(planeswalker))
    );
    let characteristics = engine
        .characteristics(planeswalker)
        .expect("attached planeswalker has characteristics");
    assert!(characteristics.is_creature());
    assert!(!characteristics.has_type("Planeswalker"));
    assert_eq!(engine.effective_power(planeswalker), Some(2));
    assert_eq!(engine.effective_toughness(planeswalker), Some(2));
}
fn prepared_luxior_trio(seed: u64) -> (GameEngine, u32, u32, u32) {
    let mut engine = luxior_engine(seed);
    let luxior = cast_fixture(&mut engine, 0, LUXIOR);
    let planeswalker = cast_fixture(&mut engine, 0, PLANESWALKER);
    let creature = relocate_to_battlefield(&mut engine, 0, CREATURE, false);
    (engine, luxior, planeswalker, creature)
}

fn add_prepared_effect(
    engine: &mut GameEngine,
    object_id: u32,
    kind: ContinuousEffectKind,
) -> ContinuousEffect {
    let effect = ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    };
    engine.state.continuous_effects.push(effect.clone());
    engine.initial_response_batch();
    effect
}

fn remove_prepared_effect(engine: &mut GameEngine, effect: &ContinuousEffect) {
    engine
        .state
        .continuous_effects
        .retain(|current| current != effect);
    engine.initial_response_batch();
}

fn make_source_creature(engine: &mut GameEngine, luxior: u32) -> ContinuousEffect {
    {
        let object = engine
            .state
            .objects
            .get_mut(&luxior)
            .expect("Luxior source");
        object.power = Some(2);
        object.toughness = Some(2);
    }
    add_prepared_effect(
        engine,
        luxior,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..TypeLineAddition::default()
        }),
    )
}

fn make_source_non_equipment(engine: &mut GameEngine, luxior: u32) -> ContinuousEffect {
    add_prepared_effect(
        engine,
        luxior,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            artifact_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: Vec::new(),
            land_types: Vec::new(),
        }),
    )
}

fn add_planeswalker_protection(engine: &mut GameEngine, planeswalker: u32) -> ContinuousEffect {
    add_prepared_effect(
        engine,
        planeswalker,
        ContinuousEffectKind::Layer6AddProtection(ProtectionQuality::CardType(
            ProtectionCardType::Artifact,
        )),
    )
}

fn change_controller(engine: &mut GameEngine, object_id: u32, from: usize, to: usize) {
    engine.state.players[from]
        .battlefield
        .retain(|existing| *existing != object_id);
    engine.state.players[to].battlefield.push(object_id);
    let controller = engine.state.players[to].id;
    let object = engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("permanent to change control");
    object.base_controller = controller;
    object.controller = controller;
    engine.initial_response_batch();
}

fn phase_out(engine: &mut GameEngine, object_id: u32, player: usize) {
    let phase_out_under = engine.state.players[player].id;
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("permanent to phase out")
        .phase_status = Some(PhasedOutStatus {
        phase_out_under,
        phase_in_due_after_departed_turn: false,
        phase_in_with: None,
        direct_attachment_snapshot: None,
    });
    engine.initial_response_batch();
}

fn same_object_leave_and_return(engine: &mut GameEngine, object_id: u32) {
    let owner = engine.state.objects[&object_id].owner;
    let old_controller = engine.state.objects[&object_id].controller;
    let owner_seat = engine
        .state
        .players
        .iter()
        .position(|player| player.id == owner)
        .expect("owner seat");
    let old_controller_seat = engine
        .state
        .players
        .iter()
        .position(|player| player.id == old_controller)
        .expect("controller seat");
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);

    engine.state.players[old_controller_seat]
        .battlefield
        .retain(|existing| *existing != object_id);
    engine.state.players[owner_seat].graveyard.push(object_id);
    {
        let object = engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("departing permanent");
        object.zone = Zone::Graveyard;
        object.attached_to = None;
        object.counters.clear();
        object.counter_timestamps.clear();
        object.tapped = false;
        object.damage = 0;
        object.phase_status = None;
    }
    engine
        .state
        .zone_change_generation
        .insert(object_id, original_generation.saturating_add(1));

    engine.state.players[owner_seat]
        .graveyard
        .retain(|existing| *existing != object_id);
    engine.state.players[owner_seat].battlefield.push(object_id);
    {
        let object = engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("returning permanent");
        object.zone = Zone::Battlefield;
        object.base_controller = owner;
        object.controller = owner;
        object.summoning_sick = false;
    }
    engine
        .state
        .zone_change_generation
        .insert(object_id, original_generation.saturating_add(2));
    engine.initial_response_batch();
}

fn published_battlefield_object(
    engine: &mut GameEngine,
    player: usize,
    object_id: u32,
) -> rv1::BattlefieldObject {
    let batch = engine.initial_response_batch();
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .get(player)
                .and_then(|player| {
                    player
                        .battlefield_objects
                        .iter()
                        .find(|object| object.object_id == object_id)
                })
                .cloned(),
            _ => None,
        })
        .expect("the current public battlefield view includes this permanent")
}

fn activate_planeswalker_equip(
    engine: &mut GameEngine,
    luxior: u32,
    planeswalker: u32,
) -> Result<RuledEventBatch, tricerules_core::EngineError> {
    give_mana(
        engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let target = target_ref(engine, planeswalker);
    apply_ability(engine, 0, luxior, 1, vec![target])
}
#[test]
fn luxior_runtime_equip_source_restrictions_apply_at_attachment_only() {
    let (mut engine, luxior, planeswalker, creature) = prepared_luxior_trio(610_402);
    let creature_restriction = make_source_creature(&mut engine, luxior);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, luxior),
        [true, true]
    );

    let plane_target = target_ref(&engine, planeswalker);
    apply_ability(&mut engine, 0, luxior, 1, vec![plane_target])
        .expect("a creature Equipment may activate its Equip planeswalker ability");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&luxior].attached_to, None);
    assert!(engine
        .characteristics(luxior)
        .is_some_and(|value| value.is_creature()));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let creature_target = target_ref(&engine, creature);
    apply_ability(&mut engine, 0, luxior, 0, vec![creature_target])
        .expect("a creature Equipment may activate its ordinary Equip ability");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&luxior].attached_to, None,
        "ordinary Equip still cannot attach while the source remains a creature"
    );

    activate_planeswalker_equip(&mut engine, luxior, planeswalker)
        .expect("the planeswalker Equip remains activatable while Luxior is a creature");
    remove_prepared_effect(&mut engine, &creature_restriction);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(planeswalker)),
        "ending the creature restriction before resolution permits attachment"
    );

    let (mut non_equipment_engine, non_equipment_luxior, non_equipment_pw, _) =
        prepared_luxior_trio(610_403);
    let type_restriction =
        make_source_non_equipment(&mut non_equipment_engine, non_equipment_luxior);
    assert!(non_equipment_engine
        .characteristics(non_equipment_luxior)
        .is_some_and(|value| value.has_type("Artifact") && !value.has_type("Equipment")));
    give_mana(
        &mut non_equipment_engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        zone_view_ability_flags(&mut non_equipment_engine, 0, non_equipment_luxior),
        [true, true],
        "losing Equipment subtype does not remove a printed activated ability"
    );

    activate_planeswalker_equip(
        &mut non_equipment_engine,
        non_equipment_luxior,
        non_equipment_pw,
    )
    .expect("a non-Equipment source can activate its printed Equip ability");
    resolve_entire_stack_two_player(&mut non_equipment_engine);
    assert_eq!(
        non_equipment_engine.state.objects[&non_equipment_luxior].attached_to,
        None
    );

    activate_planeswalker_equip(
        &mut non_equipment_engine,
        non_equipment_luxior,
        non_equipment_pw,
    )
    .expect("the Equip ability remains activated while non-Equipment");
    remove_prepared_effect(&mut non_equipment_engine, &type_restriction);
    resolve_entire_stack_two_player(&mut non_equipment_engine);
    assert_eq!(
        non_equipment_engine.state.objects[&non_equipment_luxior].attached_to,
        Some(AttachmentRecipient::Object(non_equipment_pw)),
        "restoring Equipment before resolution allows the already-activated ability to attach"
    );
}

#[test]
fn luxior_runtime_stacked_controller_is_distinct_from_source_and_post_attach_control() {
    for (case, seed) in [
        ("source_control", 610_404),
        ("source_ability_loss", 610_417),
    ] {
        let (mut engine, luxior, planeswalker, _) = prepared_luxior_trio(seed);
        let target_generation = engine.state.zone_change_generation[&planeswalker];
        if case == "source_ability_loss" {
            // Keep the newly animated planeswalker alive after Luxior loses its distinct
            // counter static. This isolates the already-stacked Equip from the target's 0/0 SBA.
            let timestamp = engine.state.command_index;
            let target = engine
                .state
                .objects
                .get_mut(&planeswalker)
                .expect("planeswalker target");
            target.counters.insert(CounterKind::PlusOnePlusOne, 1);
            target
                .counter_timestamps
                .insert(CounterKind::PlusOnePlusOne, timestamp);
        }
        activate_planeswalker_equip(&mut engine, luxior, planeswalker)
            .expect("player A activates while controlling Luxior and the target");
        let stacked_equip = engine
            .state
            .stack
            .last()
            .expect("Equip planeswalker activation is captured on the stack")
            .clone();
        assert_eq!(stacked_equip.source_permanent_id, Some(luxior));
        assert_eq!(stacked_equip.ability_index, Some(1));
        assert!(stacked_equip.activated_ability.is_some());
        assert!(stacked_equip.activated_ability_occurrence.is_some());
        match case {
            "source_control" => change_controller(&mut engine, luxior, 0, 1),
            "source_ability_loss" => {
                add_prepared_effect(
                    &mut engine,
                    luxior,
                    ContinuousEffectKind::Layer6RemoveAllAbilities,
                );
            }
            _ => unreachable!(),
        }
        if case == "source_ability_loss" {
            assert_eq!(
                zone_view_ability_flags(&mut engine, 0, luxior),
                Vec::<bool>::new(),
                "losing all abilities removes the public activation list"
            );
            let still_stacked = engine
                .state
                .stack
                .last()
                .expect("source ability loss leaves the captured Equip on the stack");
            assert_eq!(still_stacked.id, stacked_equip.id);
            assert!(still_stacked.activated_ability.is_some());
            assert!(still_stacked.activated_ability_occurrence.is_some());
        }

        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&luxior].attached_to,
            Some(AttachmentRecipient::Object(planeswalker)),
            "an already-stacked Equip still attaches after {case}"
        );
        if case == "source_ability_loss" {
            assert_eq!(engine.state.objects[&planeswalker].zone, Zone::Battlefield);
            assert_eq!(
                engine.state.zone_change_generation[&planeswalker], target_generation,
                "the attached target remains the same battlefield incarnation"
            );
            let target = engine
                .characteristics(planeswalker)
                .expect("the attached target survives state-based actions");
            assert!(target.is_creature());
            assert!(!target.has_type("Planeswalker"));
            assert_eq!(engine.effective_power(planeswalker), Some(1));
            assert_eq!(engine.effective_toughness(planeswalker), Some(1));
            assert_eq!(
                zone_view_ability_flags(&mut engine, 0, luxior),
                Vec::<bool>::new(),
                "the source still has no abilities after the stacked Equip resolves"
            );
        }
    }

    let (mut engine, luxior, planeswalker, _) = prepared_luxior_trio(610_418);
    activate_planeswalker_equip(&mut engine, luxior, planeswalker)
        .expect("player A activates while controlling Luxior and the target");
    resolve_entire_stack_two_player(&mut engine);
    change_controller(&mut engine, planeswalker, 0, 1);
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(planeswalker)),
        "a target control change alone does not detach Equipment"
    );
    assert!(engine
        .characteristics(planeswalker)
        .is_some_and(|value| value.is_creature() && !value.has_type("Planeswalker")));
}

#[test]
fn luxior_runtime_target_control_type_protection_and_phase_changes_cancel_attachment() {
    for (case, seed) in [
        ("control", 610_405),
        ("type", 610_406),
        ("protection", 610_407),
        ("target_phase", 610_408),
    ] {
        let (mut engine, luxior, planeswalker, _) = prepared_luxior_trio(seed);
        activate_planeswalker_equip(&mut engine, luxior, planeswalker)
            .expect("the current controlled planeswalker is a legal target");
        match case {
            "control" => change_controller(&mut engine, planeswalker, 0, 1),
            "type" => {
                add_prepared_effect(
                    &mut engine,
                    planeswalker,
                    ContinuousEffectKind::Layer4RemovePlaneswalker,
                );
            }
            "protection" => {
                add_planeswalker_protection(&mut engine, planeswalker);
            }
            "target_phase" => phase_out(&mut engine, planeswalker, 0),
            _ => unreachable!(),
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&luxior].attached_to, None,
            "target change {case} makes the target illegal at resolution"
        );
        assert!(engine.state.stack.is_empty());
    }

    for (case, seed) in [("source_phase", 610_409), ("source_return", 610_410)] {
        let (mut engine, luxior, planeswalker, _) = prepared_luxior_trio(seed);
        activate_planeswalker_equip(&mut engine, luxior, planeswalker)
            .expect("the source and target are current when activated");
        match case {
            "source_phase" => phase_out(&mut engine, luxior, 0),
            "source_return" => same_object_leave_and_return(&mut engine, luxior),
            _ => unreachable!(),
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&luxior].attached_to, None,
            "source change {case} makes the source incarnation unavailable"
        );
        assert!(engine.state.stack.is_empty());
    }

    let (mut engine, luxior, planeswalker, _) = prepared_luxior_trio(610_411);
    activate_planeswalker_equip(&mut engine, luxior, planeswalker)
        .expect("the original target incarnation is legal");
    same_object_leave_and_return(&mut engine, planeswalker);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&luxior].attached_to, None,
        "a same-ObjectId/new-generation target is not the stacked target"
    );
}

#[test]
fn luxior_runtime_protection_is_checked_when_targeting_and_again_at_resolution() {
    let (mut protected_before, luxior, planeswalker, _) = prepared_luxior_trio(610_412);
    add_planeswalker_protection(&mut protected_before, planeswalker);
    let ability_key = (u64::from(luxior) << 32) | 1;
    let offered = protected_before.initial_response_batch();
    let targets = &offered.legal_by_player[&0].valid_targets_by_ability[&ability_key].groups[0];
    assert!(!targets.valid_permanent_ids.contains(&planeswalker));
    give_mana(
        &mut protected_before,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mana_before = protected_before.state.players[0].mana_pool.colorless;
    let target = target_ref(&protected_before, planeswalker);
    let error = apply_ability(&mut protected_before, 0, luxior, 1, vec![target])
        .expect_err("protection from artifacts prevents targeting the planeswalker");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(protected_before.state.stack.len(), 0);
    assert_eq!(
        protected_before.state.players[0].mana_pool.colorless,
        mana_before
    );

    let (mut protected_later, luxior, planeswalker, _) = prepared_luxior_trio(610_413);
    activate_planeswalker_equip(&mut protected_later, luxior, planeswalker)
        .expect("the unprotected planeswalker is a legal target at activation");
    add_planeswalker_protection(&mut protected_later, planeswalker);
    resolve_entire_stack_two_player(&mut protected_later);
    assert_eq!(protected_later.state.objects[&luxior].attached_to, None);
    assert!(protected_later.state.stack.is_empty());
}

#[test]
fn luxior_runtime_public_characteristics_and_loyalty_use_survive_equipping_and_reequipping() {
    let (mut engine, luxior, planeswalker, creature) = prepared_luxior_trio(610_414);
    activate_planeswalker_equip(&mut engine, luxior, planeswalker)
        .expect("equip a current planeswalker");
    resolve_entire_stack_two_player(&mut engine);

    let attached_view = published_battlefield_object(&mut engine, 0, planeswalker);
    assert!(attached_view.is_creature);
    assert!(!attached_view.is_planeswalker);
    assert_eq!(attached_view.power, 2);
    assert_eq!(attached_view.toughness, 2);
    assert_eq!(attached_view.loyalty, 2);
    assert_eq!(attached_view.counters_annotation, "2 loyalty counter(s)");
    assert_eq!(
        attached_view.zone_change_generation,
        engine.state.zone_change_generation[&planeswalker]
    );
    let attached_source_view = published_battlefield_object(&mut engine, 0, luxior);
    assert!(matches!(
        attached_source_view
            .attachment_recipient
            .as_ref()
            .and_then(|recipient| recipient.recipient.as_ref()),
        Some(Recipient::ObjectId(object_id)) if *object_id == planeswalker
    ));
    assert_eq!(attached_view.activated_abilities.len(), 2);
    assert!(attached_view.activated_abilities[0].activatable);
    assert!(attached_view.activated_abilities[1].activatable);

    apply_ability(&mut engine, 0, planeswalker, 0, vec![])
        .expect("a loyalty ability remains usable after the permanent loses Planeswalker type");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&planeswalker].counter_count(CounterKind::Loyalty),
        3
    );
    let used_view = published_battlefield_object(&mut engine, 0, planeswalker);
    assert!(used_view.is_creature);
    assert_eq!(used_view.loyalty, 3);
    assert_eq!(used_view.power, 3);
    assert_eq!(used_view.toughness, 3);
    assert_eq!(used_view.counters_annotation, "3 loyalty counter(s)");
    assert!(used_view
        .activated_abilities
        .iter()
        .all(|ability| !ability.activatable));
    assert!(apply_ability(&mut engine, 0, planeswalker, 1, vec![]).is_err());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let creature_target = target_ref(&engine, creature);
    apply_ability(&mut engine, 0, luxior, 0, vec![creature_target])
        .expect("ordinary Equip moves Luxior to a creature");
    resolve_entire_stack_two_player(&mut engine);
    let restored_view = published_battlefield_object(&mut engine, 0, planeswalker);
    assert!(!restored_view.is_creature);
    assert!(restored_view.is_planeswalker);
    assert_eq!(restored_view.loyalty, 3);
    assert_eq!(
        restored_view
            .attachment_recipient
            .as_ref()
            .and_then(|recipient| recipient.recipient.as_ref()),
        None
    );
    assert!(apply_ability(&mut engine, 0, planeswalker, 1, vec![]).is_err());

    activate_planeswalker_equip(&mut engine, luxior, planeswalker)
        .expect("the planeswalker remains a legal target after re-equipping");
    resolve_entire_stack_two_player(&mut engine);
    let reequip_view = published_battlefield_object(&mut engine, 0, planeswalker);
    assert!(reequip_view.is_creature);
    assert!(!reequip_view.is_planeswalker);
    assert_eq!(reequip_view.loyalty, 3);
    assert!(apply_ability(&mut engine, 0, planeswalker, 1, vec![]).is_err());
}

#[test]
fn luxior_runtime_separates_creature_toughness_and_planeswalker_loyalty_sbas() {
    let (mut zero_toughness, luxior, planeswalker, _) = prepared_luxior_trio(610_415);
    {
        let object = zero_toughness.state.objects.get_mut(&planeswalker).unwrap();
        object.counters.clear();
        object.counter_timestamps.clear();
        object.counters.insert(CounterKind::Loyalty, 1);
        object.counter_timestamps.insert(CounterKind::Loyalty, 0);
    }
    activate_planeswalker_equip(&mut zero_toughness, luxior, planeswalker)
        .expect("one loyalty counter is enough to survive while equipped");
    resolve_entire_stack_two_player(&mut zero_toughness);
    assert!(zero_toughness
        .characteristics(planeswalker)
        .is_some_and(|value| value.is_creature()));
    assert_eq!(zero_toughness.effective_toughness(planeswalker), Some(1));
    apply_ability(&mut zero_toughness, 0, planeswalker, 1, vec![])
        .expect("the attached creature retains its loyalty ability and can spend its counter");
    assert_eq!(
        zero_toughness.state.objects[&planeswalker].zone,
        Zone::Graveyard,
        "zero toughness moves the non-planeswalker creature before its loyalty ability resolves"
    );
    assert_eq!(zero_toughness.state.objects[&luxior].attached_to, None);
    resolve_entire_stack_two_player(&mut zero_toughness);
    assert!(zero_toughness.state.stack.is_empty());

    let (mut zero_loyalty, luxior, planeswalker, creature) = prepared_luxior_trio(610_416);
    {
        let object = zero_loyalty.state.objects.get_mut(&planeswalker).unwrap();
        object.counters.clear();
        object.counter_timestamps.clear();
        object.counters.insert(CounterKind::Loyalty, 1);
        object.counter_timestamps.insert(CounterKind::Loyalty, 0);
        object.counters.insert(CounterKind::Charge, 1);
        object.counter_timestamps.insert(CounterKind::Charge, 0);
    }
    add_prepared_effect(
        &mut zero_loyalty,
        planeswalker,
        ContinuousEffectKind::Layer7bSetPt {
            power: 0,
            toughness: 2,
        },
    );
    activate_planeswalker_equip(&mut zero_loyalty, luxior, planeswalker)
        .expect("equip a one-loyalty planeswalker with independent base toughness");
    resolve_entire_stack_two_player(&mut zero_loyalty);
    assert_eq!(zero_loyalty.effective_toughness(planeswalker), Some(4));

    give_mana(
        &mut zero_loyalty,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let creature_target = target_ref(&zero_loyalty, creature);
    apply_ability(&mut zero_loyalty, 0, luxior, 0, vec![creature_target])
        .expect("activate ordinary Equip before the prepared loyalty removal");
    zero_loyalty
        .state
        .objects
        .get_mut(&planeswalker)
        .unwrap()
        .counters
        .remove(&CounterKind::Loyalty);
    zero_loyalty
        .state
        .objects
        .get_mut(&planeswalker)
        .unwrap()
        .counter_timestamps
        .remove(&CounterKind::Loyalty);
    assert_eq!(zero_loyalty.effective_toughness(planeswalker), Some(3));
    resolve_entire_stack_two_player(&mut zero_loyalty);
    assert_eq!(
        zero_loyalty.state.objects[&planeswalker].zone,
        Zone::Graveyard,
        "after detachment it regains Planeswalker and dies for zero loyalty"
    );
    assert_eq!(
        zero_loyalty.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
}
