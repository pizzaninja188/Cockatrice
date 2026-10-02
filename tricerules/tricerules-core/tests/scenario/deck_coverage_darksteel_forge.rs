//! Darksteel Forge: a live source-bound layer-six keyword grant to artifact permanents.
use crate::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, ControllerReference, EffectDuration, PermanentTypeFilter,
    TypeLineAddition, TypeLineReplacement,
};
use tricerules_cards::{CardRegistry, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const FORGE: &str = "darksteel_forge";

fn forge_engine() -> GameEngine {
    assert!(
        CardRegistry::global().get(FORGE).is_some(),
        "missing exact Darksteel Forge"
    );
    let deck = deck_with("forest", &[FORGE, "sol_ring", "ornithopter"]);
    let mut engine = GameEngine::new(2026093012, &[0, 1], 20, Some(vec![deck; 2]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn indestructible(engine: &GameEngine, object: u32) -> bool {
    engine
        .characteristics(object)
        .unwrap()
        .has_keyword(Keyword::Indestructible)
}

fn effect(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind, timestamp: u64) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp,
    });
}

fn move_forge_out(engine: &mut GameEngine) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Darksteel Forge".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn forge_departure_kills_marked_artifact_and_reentry_uses_a_fresh_incarnation() {
    let mut engine = forge_engine();
    let source = move_ready_to_battlefield(&mut engine, 0, FORGE);
    let creature = move_ready_to_battlefield(&mut engine, 0, "ornithopter");
    let generation = engine.state.zone_change_generation[&source];
    engine.state.objects.get_mut(&creature).unwrap().damage = 2;
    move_forge_out(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.zone_change_generation[&source], generation + 1);
    let returned = move_ready_to_battlefield(&mut engine, 0, FORGE);
    assert_eq!(returned, source);
    assert_eq!(engine.state.zone_change_generation[&source], generation + 2);
    assert!(indestructible(&engine, source));
    assert_eq!(
        engine
            .state
            .continuous_effects
            .iter()
            .filter(|effect| {
                effect.source_id == Some(source)
                    && matches!(
                        effect.kind,
                        ContinuousEffectKind::Layer6AddKeywordFromStatic { .. }
                    )
            })
            .count(),
        1
    );
}

#[test]
fn forge_does_not_prevent_sacrifice_or_zero_toughness() {
    let mut engine = forge_engine();
    let source = move_ready_to_battlefield(&mut engine, 0, FORGE);
    let ironworks = inject_permanent_on_battlefield(&mut engine, 0, "krark-clan_ironworks");
    assert!(indestructible(&engine, source));
    let mut command = activate_ability_with_costs(
        ironworks,
        0,
        vec![],
        vec![permanent_cost_selection(0, source)],
    );
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.expected_zone_change_generation = engine
            .state
            .zone_change_generation
            .get(&ironworks)
            .copied()
            .unwrap_or(0);
    }
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    move_ready_to_battlefield(&mut engine, 0, FORGE);
    let creature = move_ready_to_battlefield(&mut engine, 0, "ornithopter");
    assert!(indestructible(&engine, creature));
    effect(
        &mut engine,
        creature,
        ContinuousEffectKind::PtModify {
            delta_power: 0,
            delta_toughness: -2,
        },
        1000,
    );
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .unwrap();
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
}

#[test]
fn sculpting_steel_copy_of_forge_keeps_its_own_grant_after_donor_departure() {
    let mut engine = forge_engine();
    let source = move_ready_to_battlefield(&mut engine, 0, FORGE);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    inject_card_into_hand(&mut engine, 0, "sculpting_steel");
    let slot = hand_index_for_card(&engine, 0, "sculpting_steel");
    let copy = engine.state.players[0].hand[slot];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .contains(&source));
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    assert_eq!(
        engine.characteristics(copy).unwrap().names,
        ["Darksteel Forge"]
    );
    assert_eq!(engine.state.objects[&copy].card_id, "sculpting_steel");
    assert!(engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| effect.source_id == Some(copy)
            && matches!(
                effect.kind,
                ContinuousEffectKind::Layer6AddKeywordFromStatic { .. }
            )));
    move_forge_out(&mut engine);
    assert!(indestructible(&engine, copy));
    assert!(indestructible(&engine, ring));
    effect(
        &mut engine,
        copy,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
        1000,
    );
    assert!(!indestructible(&engine, ring));
}

#[test]
fn forge_actual_cast_pays_nine_projects_the_keyword_and_rejects_invented_activation() {
    let mut engine = forge_engine();
    inject_card_into_hand(&mut engine, 0, FORGE);
    let slot = hand_index_for_card(&engine, 0, FORGE);
    let source = engine.state.players[0].hand[slot];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 8,
            ..Default::default()
        },
    );
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect_err("nine mana required");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    let batch = engine.initial_response_batch();
    let projected = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source),
            _ => None,
        })
        .unwrap();
    assert!(projected.keywords.contains(&"Indestructible".to_string()));
    assert!(projected.activated_abilities.is_empty());
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .expect_err("a static grant is not an activated ability");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
}

#[test]
fn forge_source_and_recipient_removal_have_distinct_ordering_and_restore_on_expiry() {
    for timestamp in [0, 1_000_000] {
        let mut engine = forge_engine();
        let source = move_ready_to_battlefield(&mut engine, 0, FORGE);
        let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
        effect(
            &mut engine,
            source,
            ContinuousEffectKind::Layer6RemoveAllAbilities,
            timestamp,
        );
        assert!(!indestructible(&engine, source));
        assert!(!indestructible(&engine, ring));
        end_active_turn(&mut engine, 0);
        assert!(indestructible(&engine, source));
        assert!(indestructible(&engine, ring));
    }
    for (timestamp, expected) in [(0, true), (1_000_000, false)] {
        let mut engine = forge_engine();
        move_ready_to_battlefield(&mut engine, 0, FORGE);
        let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
        effect(
            &mut engine,
            ring,
            ContinuousEffectKind::Layer6RemoveAllAbilities,
            timestamp,
        );
        assert_eq!(indestructible(&engine, ring), expected);
    }
}

#[test]
fn forge_scope_follows_types_source_control_and_recipient_control() {
    let mut engine = forge_engine();
    let source = move_ready_to_battlefield(&mut engine, 0, FORGE);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let opponent = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    effect(
        &mut engine,
        bear,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
        }),
        100,
    );
    assert!(indestructible(&engine, bear));
    effect(
        &mut engine,
        ring,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Enchantment],
            creature_types: vec![],
        }),
        101,
    );
    assert!(!indestructible(&engine, ring));
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Enchantment],
            creature_types: vec![],
        }),
        102,
    );
    assert!(
        indestructible(&engine, bear),
        "losing Artifact does not remove Forge's ability"
    );
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        103,
    );
    assert!(!indestructible(&engine, bear));
    assert!(indestructible(&engine, opponent));
    effect(
        &mut engine,
        bear,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        104,
    );
    assert!(indestructible(&engine, bear));
}

#[test]
fn forge_prevents_destroy_and_lethal_damage_but_source_ability_removal_exposes_damage() {
    let mut engine = forge_engine();
    let source = move_ready_to_battlefield(&mut engine, 0, FORGE);
    let creature = move_ready_to_battlefield(&mut engine, 0, "ornithopter");
    let slot = {
        inject_card_into_hand(&mut engine, 0, "disenchant");
        hand_index_for_card(&engine, 0, "disenchant")
    };
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Battlefield,
        "destroy was legal but ineffective"
    );
    engine.state.objects.get_mut(&creature).unwrap().damage = 2;
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&creature].damage, 2);
    // Remove the source's generating ability; the next SBA now sees the still-marked damage.
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
        1000,
    );
    engine.apply_command(1, &pass()).unwrap();
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
}

#[test]
fn witness_protection_started_layer_four_component_keeps_removing_forge_ability_after_aura_ability_removal(
) {
    let mut engine = forge_engine();
    let source = move_ready_to_battlefield(&mut engine, 0, FORGE);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec![],
        }),
        100,
    );
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer7bSetPt {
            power: 2,
            toughness: 2,
        },
        0,
    );
    let aura = inject_card_into_hand(&mut engine, 0, "witness_protection");
    let slot = hand_index_for_card(&engine, 0, "witness_protection");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert!(!indestructible(&engine, ring));
    assert!(!engine.characteristics(source).unwrap().is_artifact());
    inject_card_into_hand(&mut engine, 0, FORGE);
    move_ready_to_battlefield(&mut engine, 0, FORGE);
    assert!(indestructible(&engine, ring));
    assert!(!indestructible(&engine, source));
    effect(
        &mut engine,
        aura,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
        1_000_000,
    );
    assert!(indestructible(&engine, ring));
    assert!(
        !indestructible(&engine, source),
        "the other Forge excludes this nonartifact"
    );
    assert!(
        !engine.characteristics(source).unwrap().is_artifact(),
        "earlier-layer type changes must still feed another Forge's artifact scope"
    );
    assert_eq!(engine.characteristics(source).unwrap().toughness, Some(1));
    assert_eq!(
        engine.characteristics(source).unwrap().names,
        ["Legitimate Businessperson"]
    );
    assert_eq!(
        engine.characteristics(source).unwrap().colors,
        [
            tricerules_cards::primitives::Color::White,
            tricerules_cards::primitives::Color::Green
        ]
    );
}

#[test]
fn forge_grants_to_itself_artifact_creatures_and_noncreatures_but_not_opponents() {
    let mut engine = forge_engine();
    let source = move_ready_to_battlefield(&mut engine, 0, FORGE);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let creature = move_ready_to_battlefield(&mut engine, 0, "ornithopter");
    let ordinary = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    assert!(indestructible(&engine, source));
    assert!(indestructible(&engine, ring));
    assert!(indestructible(&engine, creature));
    assert!(!indestructible(&engine, ordinary));
    assert!(!indestructible(&engine, opponent));
}
