//! Prepared animation capability fixtures. No actual Vronos definition or admission.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use std::sync::OnceLock;
use tricerules_cards::{CardRegistry, CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

const SOURCE: &str = "animation_fixture";
const FIXTURE: &str = r#"(id: "animation_fixture", name: "Animation Fixture", face_id: "animation_fixture", types: ["Planeswalker"], loyalty: 7,
    activated_abilities: [(ability_id: "animate", presentation: Fallback, costs: [Loyalty(-7)],
        targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose an artifact you control", effect_indices: [0,1,2,3,4])])),
        effect: [
            ApplyPermanentModifier(subject: Chosen((kind: AnyPermanent, controller: You, permanent_types: [Artifact])), modifier: AddTypes((card_types: [Artifact, Creature])), duration: Indefinite),
            ApplyPermanentModifier(subject: Chosen((kind: AnyPermanent, controller: You, permanent_types: [Artifact])), modifier: SetCreatureTypes(["Construct"]), duration: Indefinite),
            ApplyPermanentModifier(subject: Chosen((kind: AnyPermanent, controller: You, permanent_types: [Artifact])), modifier: SetBasePowerToughness(power: 9, toughness: 9), duration: Indefinite),
            ApplyPermanentModifier(subject: Chosen((kind: AnyPermanent, controller: You, permanent_types: [Artifact])), modifier: GrantKeywords([Vigilance, Indestructible]), duration: Indefinite),
            ApplyPermanentModifier(subject: Chosen((kind: AnyPermanent, controller: You, permanent_types: [Artifact])), modifier: GrantCantBeBlocked, duration: Indefinite),
        ]
    )])"#;

fn game() -> (GameEngine, u32) {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| CardRegistry::from_chunks_and_tokens(&[
        FIXTURE,
        r#"(id: "relic_fixture", name: "Relic Fixture", face_id: "relic_fixture", mana_cost: "{G}", supertypes: ["Legendary", "Snow"], types: ["Artifact", "Creature", "Elf", "Equipment"], power: 2, toughness: 2, keywords: [Flying])"#,
        r#"(id: "plain_relic", name: "Plain Relic", face_id: "plain_relic", types: ["Artifact"])"#,
        r#"(id: "equipment_relic", name: "Equipment Relic", face_id: "equipment_relic", types: ["Artifact", "Equipment"])"#,
        r#"(id: "island_enchantment", name: "Island Enchantment", face_id: "island_enchantment", types: ["Enchantment", "Land", "Island"])"#,
        r#"(id: "erase_fixture", name: "Erase Fixture", face_id: "erase_fixture", mana_cost: "{0}", types: ["Instant"], spell_effect: [RemoveAllAbilities(subject: Chosen((kind: Creature)), duration: Indefinite)])"#,
        r#"(id: "grant_fixture", name: "Grant Fixture", face_id: "grant_fixture", mana_cost: "{0}", types: ["Instant"], spell_effect: [ApplyPermanentModifier(subject: Chosen((kind: Creature)), modifier: GrantCantBeBlocked, duration: Indefinite)])"#,
        r#"(id: "base_fixture", name: "Base Fixture", face_id: "base_fixture", mana_cost: "{0}", types: ["Instant"], spell_effect: [ApplyPermanentModifier(subject: Chosen((kind: Creature)), modifier: SetBasePowerToughness(power: 1, toughness: 1), duration: Indefinite)])"#,
        r#"(id: "grow_fixture", name: "Grow Fixture", face_id: "grow_fixture", mana_cost: "{0}", types: ["Instant"], spell_effect: [PumpTarget(subject: Chosen((kind: Creature)), power: 3, toughness: 3)])"#,
        include_str!("../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
        include_str!("../../../tricerules-cards/data/liquimetal_coating.ron"),
        include_str!("../../../tricerules-cards/data/rings_of_brighthearth.ron"),
    ], &[]).unwrap());
    let mut engine = GameEngine::new(
        registry,
        731_371,
        &[17, 29, 41],
        20,
        Some(vec![vec!["forest".into(); 40]; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = permanent(&mut engine, 0, SOURCE);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 7);
    (engine, source)
}

fn permanent(engine: &mut GameEngine, index: usize, id: &str) -> u32 {
    let object = if id == "grizzly_bears" || id == "relic_fixture" {
        inject_creature_on_battlefield(engine, index, id)
    } else {
        inject_permanent_on_battlefield(engine, index, id)
    };
    engine
        .state
        .zone_change_generation
        .entry(object)
        .or_insert(0);
    object
}

fn animate(engine: &mut GameEngine, source: u32, target: u32) {
    let command = activate_ability_for(engine, source, 0, target_object(target));
    engine.apply_scenario_command(17, &command).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    pass_priority_round(engine);
}

fn spell(engine: &mut GameEngine, id: &str, target: u32) {
    inject_card_into_hand(engine, 0, id);
    let command = cast_spell(hand_index_for_card(engine, 0, id), target_object(target));
    engine.apply_scenario_command(17, &command).unwrap();
    pass_priority_round(engine);
}

fn move_card(engine: &mut GameEngine, name: &str, zone: rv1::DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                    target_player_id: 17,
                    dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
}

fn cant_be_blocked(engine: &mut GameEngine, target: u32) -> bool {
    zone_view_rules_annotation_labels(engine, 0, target).contains(&"Can't be blocked".into())
}

#[test]
fn vronos_animation_preserves_other_types_colors_abilities_and_sets_construct_and_base_stats() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "relic_fixture");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    animate(&mut engine, source, target);
    let current = engine.characteristics(target).unwrap();
    for card_type in ["Artifact", "Creature", "Equipment", "Construct"] {
        assert!(current.has_type(card_type));
    }
    assert!(!current.has_type("Elf"));
    assert_eq!(current.supertypes, ["Legendary", "Snow"]);
    assert_eq!(current.colors, [tricerules_cards::Color::Green]);
    for keyword in [Keyword::Flying, Keyword::Vigilance, Keyword::Indestructible] {
        assert!(current.has_keyword(keyword));
    }
    assert!(!current.has_keyword(Keyword::Haste));
    assert_eq!((current.power, current.toughness), (Some(11), Some(11)));
    assert!(cant_be_blocked(&mut engine, target));
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    end_active_turn(&mut engine, 17);
    assert_eq!(
        (
            engine.characteristics(target).unwrap().power,
            engine.characteristics(target).unwrap().toughness
        ),
        (Some(11), Some(11))
    );
    assert!(cant_be_blocked(&mut engine, target));
}

#[test]
fn vronos_animation_layer_six_removal_and_later_grant_follow_timestamps() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "relic_fixture");
    animate(&mut engine, source, target);
    spell(&mut engine, "erase_fixture", target);
    let current = engine.characteristics(target).unwrap();
    for keyword in [Keyword::Flying, Keyword::Vigilance, Keyword::Indestructible] {
        assert!(!current.has_keyword(keyword));
    }
    assert!(current.has_type("Construct"));
    assert_eq!(current.power, Some(9));
    assert!(!cant_be_blocked(&mut engine, target));
    spell(&mut engine, "grant_fixture", target);
    assert!(cant_be_blocked(&mut engine, target));
    assert!(!engine
        .characteristics(target)
        .unwrap()
        .has_keyword(Keyword::Indestructible));
    spell(&mut engine, "erase_fixture", target);
    assert!(!cant_be_blocked(&mut engine, target));
}

#[test]
fn vronos_animation_base_setting_counters_and_pumps_use_the_existing_layers() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "relic_fixture");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    animate(&mut engine, source, target);
    spell(&mut engine, "grow_fixture", target);
    assert_eq!(engine.characteristics(target).unwrap().power, Some(14));
    spell(&mut engine, "base_fixture", target);
    assert_eq!(engine.characteristics(target).unwrap().power, Some(6));
    end_active_turn(&mut engine, 17);
    assert_eq!(engine.characteristics(target).unwrap().power, Some(3));
}

#[test]
fn vronos_animation_retains_artifact_after_coating_expires_and_preserves_land_and_enchantment() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "island_enchantment");
    let coating = permanent(&mut engine, 0, "liquimetal_coating");
    let command = activate_ability_for(&engine, coating, 0, target_object(target));
    engine.apply_scenario_command(17, &command).unwrap();
    pass_priority_round(&mut engine);
    assert!(engine.characteristics(target).unwrap().is_artifact());
    animate(&mut engine, source, target);
    end_active_turn(&mut engine, 17);
    let current = engine.characteristics(target).unwrap();
    for card_type in [
        "Artifact",
        "Creature",
        "Construct",
        "Land",
        "Island",
        "Enchantment",
    ] {
        assert!(current.has_type(card_type));
    }
    assert_eq!(current.power, Some(9));
}

#[test]
fn vronos_animation_blink_discards_modifiers_and_old_counter_state() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "relic_fixture");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    animate(&mut engine, source, target);
    let before = engine.state.zone_change_generation[&target];
    move_card(&mut engine, "Relic Fixture", rv1::DevZone::Graveyard);
    move_card(&mut engine, "Relic Fixture", rv1::DevZone::Battlefield);
    assert!(engine.state.zone_change_generation[&target] > before);
    let current = engine.characteristics(target).unwrap();
    assert!(current.has_type("Elf") && !current.has_type("Construct"));
    assert_eq!(current.power, Some(2));
    assert!(current.has_keyword(Keyword::Flying));
    assert!(!current.has_keyword(Keyword::Indestructible));
    assert!(!cant_be_blocked(&mut engine, target));
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn vronos_animation_rejects_illegal_targets_atomically_and_fizzles_after_blink() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "plain_relic");
    let opponent = permanent(&mut engine, 1, "plain_relic");
    let nonartifact = permanent(&mut engine, 0, "forest");
    let mut stale = target_object(target);
    stale[0].expected_zone_change_generation = Some(99);
    for targets in [
        vec![],
        target_object(opponent),
        target_object(nonartifact),
        stale,
    ] {
        let command = activate_ability_for(&engine, source, 0, targets);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_scenario_command(17, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    let command = activate_ability_for(&engine, source, 0, target_object(target));
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_scenario_command(29, &command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine.apply_scenario_command(17, &command).unwrap();
    move_card(&mut engine, "Plain Relic", rv1::DevZone::Graveyard);
    move_card(&mut engine, "Plain Relic", rv1::DevZone::Battlefield);
    pass_priority_round(&mut engine);
    assert!(!engine.characteristics(target).unwrap().is_creature());
    assert!(!cant_be_blocked(&mut engine, target));
}

fn attackers_step(engine: &mut GameEngine) {
    engine
        .apply_scenario_command(17, &primitive_yield())
        .unwrap();
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn assignments(engine: &GameEngine, attackers: &[u32]) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DeclareAttackers(rv1::DeclareAttackers {
            assignments: attackers
                .iter()
                .map(|id| rv1::AttackAssignment {
                    attacker_object_id: *id,
                    attacker_zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(id)
                        .copied()
                        .unwrap_or(0),
                    defender: Some(rv1::TargetRef {
                        object_id: 29,
                        kind: rv1::TargetRefKind::Player as i32,
                        ..Default::default()
                    }),
                    defending_player_id: 29,
                    ..Default::default()
                })
                .collect(),
        })),
    }
}

#[test]
fn vronos_animation_granted_ability_controls_block_offers_and_block_validation() {
    for changes in 0..3 {
        let (mut engine, source) = game();
        let target = permanent(&mut engine, 0, "plain_relic");
        let ordinary = permanent(&mut engine, 0, "grizzly_bears");
        let blocker = permanent(&mut engine, 1, "grizzly_bears");
        animate(&mut engine, source, target);
        if changes > 0 {
            spell(&mut engine, "erase_fixture", target);
        }
        if changes > 1 {
            spell(&mut engine, "grant_fixture", target);
        }
        attackers_step(&mut engine);
        let command = assignments(&engine, &[target, ordinary]);
        engine.apply_scenario_command(17, &command).unwrap();
        assert_eq!(engine.state.objects[&target].tapped, changes != 0);
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
        let pair = rv1::BlockPair {
            attacker_id: target,
            blocker_id: blocker,
        };
        let batch = engine.initial_response_batch();
        assert_eq!(
            batch.legal_by_player[&29].legal_block_pairs.contains(&pair),
            changes == 1
        );
        let before = engine.diagnostic_snapshot().unwrap();
        let result = engine.apply_scenario_command(29, &declare_blockers(vec![pair]));
        if changes == 1 {
            result.unwrap();
        } else {
            assert!(result.is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        }
    }
}

#[test]
fn vronos_animation_does_not_grant_haste_to_a_newly_controlled_artifact() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "plain_relic");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .summoning_sick = true;
    permanent(&mut engine, 0, "grizzly_bears");
    animate(&mut engine, source, target);
    attackers_step(&mut engine);
    let batch = engine.initial_response_batch();
    assert!(!batch.legal_by_player[&17]
        .selectable_attacker_ids
        .contains(&target));
    let command = assignments(&engine, &[target]);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_scenario_command(17, &command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn vronos_animation_rings_copy_animates_one_new_shared_target_after_source_dies() {
    let (mut engine, source) = game();
    let first = permanent(&mut engine, 0, "plain_relic");
    let second = permanent(&mut engine, 0, "plain_relic");
    permanent(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = activate_ability_for(&engine, source, 0, target_object(first));
    engine.apply_scenario_command(17, &command).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            17,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .unwrap();
    submit_mana_resolution_decision(&mut engine, 17, ResolutionChoiceDecision::PayMana).unwrap();
    engine
        .apply_scenario_command(17, &submit_resolution_choice(vec![second]))
        .unwrap();
    assert!(engine.state.stack.last().unwrap().is_copy);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    for target in [first, second] {
        let current = engine.characteristics(target).unwrap();
        assert!(current.has_type("Construct") && current.is_artifact() && current.is_creature());
        assert_eq!((current.power, current.toughness), (Some(9), Some(9)));
        assert!(
            current.has_keyword(Keyword::Vigilance) && current.has_keyword(Keyword::Indestructible)
        );
        assert!(cant_be_blocked(&mut engine, target));
    }
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn vronos_animation_of_coated_face_down_object_adds_only_the_resolved_public_abilities() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "relic_fixture");
    engine.state.objects.get_mut(&target).unwrap().face_down = true;
    assert!(!engine.characteristics(target).unwrap().is_artifact());
    let coating = permanent(&mut engine, 0, "liquimetal_coating");
    let command = activate_ability_for(&engine, coating, 0, target_object(target));
    engine.apply_scenario_command(17, &command).unwrap();
    pass_priority_round(&mut engine);
    animate(&mut engine, source, target);
    let current = engine.characteristics(target).unwrap();
    assert!(current.has_type("Construct") && current.is_artifact());
    assert!(!current.has_type("Elf") && !current.has_keyword(Keyword::Flying));
    assert_eq!((current.power, current.toughness), (Some(9), Some(9)));
    assert!(
        current.has_keyword(Keyword::Vigilance) && current.has_keyword(Keyword::Indestructible)
    );
    assert!(cant_be_blocked(&mut engine, target));
    assert!(engine.state.objects[&target].face_down);
    assert!(current.names.is_empty());
    spell(&mut engine, "erase_fixture", target);
    assert!(!cant_be_blocked(&mut engine, target));
}

#[test]
fn vronos_animation_keeps_equipment_subtype_but_unattaches_the_new_creature() {
    let (mut engine, source) = game();
    let target = permanent(&mut engine, 0, "equipment_relic");
    let host = permanent(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&target).unwrap().attached_to =
        Some(tricerules_core::state::AttachmentRecipient::Object(host));
    animate(&mut engine, source, target);
    let current = engine.characteristics(target).unwrap();
    for card_type in ["Artifact", "Creature", "Construct", "Equipment"] {
        assert!(current.has_type(card_type));
    }
    assert!(engine.state.objects[&target].attached_to.is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.characteristics(host).unwrap().power, Some(2));
}
