//! Issue #215: Crew actions and Lumbering Worldwagon's characteristic-defined power.
//!
//! Oracle verified 2026-09-03. Governing rules: CR 208.2a, 208.3, 301.7,
//! 604.3, 613.1d, 613.4a-c, and 702.122.

use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_cards::{
    AbilityCost, CardRegistry, CharacteristicDefiningAbility, ObjectContributionKind,
    ObjectPaymentConstraint,
};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, dev_command, ruled_command::Cmd, ChoiceKind, CostChoiceKind,
    CostObjectRef, CostObjectRefs, CostSelection, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn crew_selection(engine: &GameEngine, objects: &[u32]) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|object_id| CostObjectRef {
                    object_id: *object_id,
                    zone_change_generation: generation(engine, *object_id),
                })
                .collect(),
        })),
    }
}

fn activate_crew(engine: &GameEngine, wagon: u32, objects: &[u32]) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(wagon, 0, vec![], vec![crew_selection(engine, objects)]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(engine, wagon);
    command
}

fn engine_with_worldwagon(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["lumbering_worldwagon"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("Lumbering Worldwagon and its Crew ability must be registered");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn add_effect(engine: &mut GameEngine, oid: u32, kind: ContinuousEffectKind) {
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

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn resolve_top_to_choice(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    answer_trigger_order_in_engine_order(engine);
    let first = engine.state.priority_player_id();
    let second = engine
        .state
        .players
        .iter()
        .map(|player| player.id)
        .find(|player| *player != first)
        .unwrap();
    engine.apply_command(first, &pass()).unwrap();
    engine.apply_command(second, &pass()).unwrap()
}

#[test]
fn issue_215_worldwagon_has_exact_cda_crew_and_trigger_shapes() {
    let face = CardRegistry::global()
        .get("lumbering_worldwagon")
        .expect("Lumbering Worldwagon is registered")
        .primary_face();
    assert_eq!(face.types, ["Artifact", "Vehicle"]);
    assert_eq!((face.power, face.toughness), (Some(0), Some(4)));
    assert_eq!(face.characteristic_defining_abilities.len(), 1);
    assert!(matches!(
        &face.characteristic_defining_abilities[0].definition,
        CharacteristicDefiningAbility::CountScaledPowerToughness {
            power_per_match: 1,
            toughness_per_match: 0,
            ..
        }
    ));
    assert_eq!(face.triggered_abilities.len(), 2);
    assert_eq!(face.activated_abilities.len(), 1);
    assert!(matches!(
        &face.activated_abilities[0].costs[0],
        AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::AggregateMinimum {
                minimum: 4,
                contribution: ObjectContributionKind::CurrentPower,
            },
            exclude_source: true,
            ..
        }
    ));
}

#[test]
fn worldwagon_cda_is_live_and_follows_layer_7_order() {
    let mut engine = engine_with_worldwagon(215_001);
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut engine, 0, "forest");
    }
    inject_permanent_on_battlefield(&mut engine, 1, "island");
    let wagon = engine
        .state
        .objects
        .iter()
        .find_map(|(&oid, object)| (object.card_id == "lumbering_worldwagon").then_some(oid))
        .expect("Worldwagon object");

    let outside_battlefield = engine
        .characteristics(wagon)
        .expect("Worldwagon characteristics");
    assert_eq!(
        (outside_battlefield.power, outside_battlefield.toughness),
        (Some(3), Some(4)),
        "the CDA works outside the battlefield"
    );

    let moved = relocate_to_battlefield(&mut engine, 0, "lumbering_worldwagon", false);
    assert_eq!(moved, wagon);
    let vehicle = engine.characteristics(wagon).unwrap();
    assert!(!vehicle.is_creature());
    assert_eq!((vehicle.power, vehicle.toughness), (None, None));

    add_effect(
        &mut engine,
        wagon,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec![],
        }),
    );
    let animated = engine.characteristics(wagon).unwrap();
    assert_eq!((animated.power, animated.toughness), (Some(3), Some(4)));

    engine.state.objects.get_mut(&wagon).unwrap().face_down = true;
    let face_down = engine.characteristics(wagon).unwrap();
    assert_eq!((face_down.power, face_down.toughness), (Some(2), Some(2)));
    engine.state.objects.get_mut(&wagon).unwrap().face_down = false;

    add_effect(
        &mut engine,
        wagon,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    let ability_removed = engine.characteristics(wagon).unwrap();
    assert_eq!(
        (ability_removed.power, ability_removed.toughness),
        (Some(0), Some(4)),
        "a layer-6 remove-all effect suppresses the later P/T CDA"
    );
    engine.state.continuous_effects.pop();

    engine
        .state
        .objects
        .get_mut(&wagon)
        .unwrap()
        .base_controller = 1;
    assert_eq!(
        engine.characteristics(wagon).unwrap().power,
        Some(1),
        "the CDA counts lands controlled by the derived controller"
    );
    engine
        .state
        .objects
        .get_mut(&wagon)
        .unwrap()
        .base_controller = 0;

    add_effect(
        &mut engine,
        wagon,
        ContinuousEffectKind::Layer7bSetPt {
            power: 7,
            toughness: 7,
        },
    );
    add_effect(
        &mut engine,
        wagon,
        ContinuousEffectKind::PtModify {
            delta_power: 2,
            delta_toughness: 1,
        },
    );
    let timestamp = engine.state.command_index;
    engine.state.objects.get_mut(&wagon).unwrap().add_counters(
        tricerules_cards::CounterKind::PlusOnePlusOne,
        1,
        timestamp,
    );
    let layered = engine.characteristics(wagon).unwrap();
    assert_eq!(
        (layered.power, layered.toughness),
        (Some(10), Some(9)),
        "layer 7b setters and layer 7c modifiers apply after the CDA"
    );
}

#[test]
fn crew_reuses_generation_bound_aggregate_tap_payment_and_adds_only_creature() {
    let mut engine = engine_with_worldwagon(215_002);
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut engine, 0, "forest");
    }
    let wagon = relocate_to_battlefield(&mut engine, 0, "lumbering_worldwagon", false);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&first).unwrap().summoning_sick = true;
    engine
        .state
        .objects
        .get_mut(&second)
        .unwrap()
        .summoning_sick = true;

    let legal = engine.initial_response_batch();
    let ability_key = u64::from(wagon) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&ability_key];
    assert!(choices.non_mana_costs_payable, "{choices:?}");
    assert_eq!(choices.choices.len(), 1);
    let tap = &choices.choices[0];
    assert_eq!(tap.kind(), CostChoiceKind::Tap);
    assert!(!tap.candidate_ids.contains(&wagon));
    assert_eq!(tap.aggregate_minimum.as_ref().unwrap().minimum, 4);
    assert_eq!(
        tap.candidate_objects
            .iter()
            .filter(
                |candidate| [first, second].contains(&candidate.object.as_ref().unwrap().object_id)
            )
            .map(|candidate| candidate.contribution)
            .sum::<i64>(),
        4
    );

    assert!(engine
        .apply_command(0, &activate_crew(&engine, wagon, &[first]),)
        .is_err());
    assert!(!engine.state.objects[&first].tapped);
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(0, &activate_crew(&engine, wagon, &[first, second]))
        .expect("summoning-sick creatures may pay Crew 4 together");
    assert!(engine.state.objects[&first].tapped && engine.state.objects[&second].tapped);
    assert!(!engine.state.objects[&wagon].tapped);
    assert!(!engine.characteristics(wagon).unwrap().is_creature());
    assert_eq!(
        (
            engine.characteristics(wagon).unwrap().power,
            engine.characteristics(wagon).unwrap().toughness,
        ),
        (None, None),
        "Crew changes the Vehicle only when the ability resolves"
    );

    resolve_entire_stack_two_player(&mut engine);
    let crewed = engine.characteristics(wagon).unwrap();
    assert!(crewed.has_type("Artifact") && crewed.has_type("Vehicle") && crewed.is_creature());
    assert_eq!((crewed.power, crewed.toughness), (Some(3), Some(4)));
    inject_permanent_on_battlefield(&mut engine, 0, "forest");
    assert_eq!(engine.characteristics(wagon).unwrap().power, Some(4));

    end_active_turn(&mut engine, 0);
    let expired = engine.characteristics(wagon).unwrap();
    assert!(!expired.is_creature());
    assert_eq!((expired.power, expired.toughness), (None, None));
}

#[test]
fn crew_effect_does_not_follow_worldwagon_to_a_new_generation() {
    let mut engine = engine_with_worldwagon(215_004);
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut engine, 0, "forest");
    }
    let wagon = relocate_to_battlefield(&mut engine, 0, "lumbering_worldwagon", false);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    engine
        .apply_command(0, &activate_crew(&engine, wagon, &[first, second]))
        .expect("activate Crew before the source changes zones");
    let old_generation = generation(&engine, wagon);
    engine.enable_dev_commands();
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Lumbering Worldwagon".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move Worldwagon before Crew resolves");
    assert!(generation(&engine, wagon) > old_generation);

    resolve_entire_stack_two_player(&mut engine);
    let returned = move_ready_to_battlefield(&mut engine, 0, "lumbering_worldwagon");
    assert_eq!(returned, wagon);
    let new_object = engine.characteristics(wagon).unwrap();
    assert!(!new_object.is_creature());
    assert_eq!((new_object.power, new_object.toughness), (None, None));
}

#[test]
fn a_copy_keeps_worldwagons_cda_but_not_its_resolved_crew_effect() {
    let mut engine = engine_with_worldwagon(215_005);
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut engine, 0, "forest");
    }
    let wagon = relocate_to_battlefield(&mut engine, 0, "lumbering_worldwagon", false);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &activate_crew(&engine, wagon, &[first, second]))
        .expect("Crew Worldwagon before copying it");
    resolve_entire_stack_two_player(&mut engine);

    inject_card_into_hand(&mut engine, 0, "cackling_counterpart");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(wagon)))
        .expect("copy the currently crewed Worldwagon");
    engine.apply_command(0, &pass()).unwrap();
    let copied = engine.apply_command(1, &pass()).unwrap();
    let token = token_created_events(&copied)[0].object_id;

    let printed_copy = engine.characteristics(token).unwrap();
    assert!(printed_copy.has_type("Artifact") && printed_copy.has_type("Vehicle"));
    assert!(!printed_copy.is_creature());
    assert_eq!((printed_copy.power, printed_copy.toughness), (None, None));
    assert!(matches!(
        &engine.state.objects[&token]
            .token_origin
            .as_ref()
            .expect("copy snapshot")
            .face
            .characteristic_defining_abilities[0]
            .definition,
        CharacteristicDefiningAbility::CountScaledPowerToughness { .. }
    ));

    add_effect(
        &mut engine,
        token,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec![],
        }),
    );
    let animated_copy = engine.characteristics(token).unwrap();
    assert_eq!(
        (animated_copy.power, animated_copy.toughness),
        (Some(3), Some(4)),
        "the copied CDA evaluates from the copy's current controller"
    );
}

#[test]
fn worldwagon_entry_and_attack_each_offer_the_same_optional_basic_search() {
    let mut engine = engine_with_worldwagon(215_003);
    let wagon = move_ready_to_battlefield(&mut engine, 0, "lumbering_worldwagon");

    let optional = resolve_top_to_choice(&mut engine);
    let branch = find_resolution_choice(&optional).expect("optional entry search");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (0, 1));
    let search = engine
        .apply_command(0, &select_branch(0))
        .expect("choose the entry search branch");
    let choice = find_resolution_choice(&search).expect("basic land search");
    let basic = *choice
        .candidate_object_ids
        .first()
        .expect("the Forest-filled deck has a basic land");
    engine
        .apply_command(0, &submit_resolution_choice(vec![basic]))
        .expect("put the searched basic land onto the battlefield");
    assert_eq!(
        engine.state.objects[&basic].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&basic].tapped);

    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &activate_crew(&engine, wagon, &[first, second]))
        .expect("Crew before combat");
    resolve_entire_stack_two_player(&mut engine);
    engine
        .apply_command(0, &primitive_yield())
        .expect("advance to beginning of combat");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![wagon]))
        .expect("attack with the crewed Vehicle");

    let optional = resolve_top_to_choice(&mut engine);
    let branch = find_resolution_choice(&optional).expect("optional attack search");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (0, 1));
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("decline the attack search");
    assert!(engine.state.pending_resolution.is_none());
}
