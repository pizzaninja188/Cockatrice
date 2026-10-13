//! Bello's turn-bound animation and combat-damage trigger.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineReplacement,
};
use tricerules_cards::Keyword;
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{DevCommand, DevPutCardInZone, DevZone};

const BELLO: &str = "bello,_bard_of_the_brambles";

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("Bello scenario game");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put(engine: &mut GameEngine, player: i32, card_name: &str, card_id: &str) -> u32 {
    let command = dev(
        player,
        Dev::PutCardInZone(DevPutCardInZone {
            card_name: card_name.to_string(),
            zone: DevZone::Battlefield as i32,
            ready: true,
        }),
    );
    engine
        .apply_scenario_command(player, &command)
        .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(engine, player as usize, card_id)
}

fn dev(target: i32, payload: Dev) -> tricerules_proto::ruled::v1::RuledCommand {
    tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(tricerules_proto::ruled::v1::ruled_command::Cmd::DevCommand(
            DevCommand {
                target_player_id: target,
                dev: Some(payload),
            },
        )),
    }
}

fn move_to_hand(engine: &mut GameEngine, player: i32, card_name: &str) {
    let command = dev(
        player,
        Dev::MoveCard(tricerules_proto::ruled::v1::DevMoveCard {
            card_name: card_name.to_string(),
            zone: DevZone::Hand as i32,
            ready: false,
        }),
    );
    engine
        .apply_scenario_command(player, &command)
        .unwrap_or_else(|error| panic!("move {card_name} to hand: {error:?}"));
}

fn deal_unblocked_combat_damage(engine: &mut GameEngine, attacker: u32) {
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main one to beginning of combat");
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_scenario_command(1, &pass())
        .expect("nonactive player passes");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the Bello-animated artifact as an attacker");
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_scenario_command(1, &pass())
        .expect("no blockers are declared");
    engine
        .apply_scenario_command(0, &pass())
        .expect("combat damage passes");
    engine
        .apply_scenario_command(1, &pass())
        .expect("combat damage resolves");
    assert_eq!(engine.state.turn_step, TurnStep::CombatDamage);
}

#[test]
fn bello_animates_qualifying_artifacts_and_enchantments_only_during_its_controllers_turn() {
    let mut engine = setup(12_460_001);
    let bello = put(&mut engine, 0, "Bello, Bard of the Brambles", BELLO);
    let artifact = put(&mut engine, 0, "Gilded Lotus", "gilded_lotus");
    let enchantment = put(&mut engine, 0, "Doubling Season", "doubling_season");
    assert_eq!(engine.characteristics(bello).unwrap().power, Some(3));

    for permanent in [artifact, enchantment] {
        let characteristics = engine
            .characteristics(permanent)
            .expect("Bello recipient characteristics");
        assert!(characteristics.is_creature());
        assert!(characteristics.has_type("Elemental"));
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(4), Some(4))
        );
        assert!(characteristics.has_keyword(Keyword::Indestructible));
        assert!(characteristics.has_keyword(Keyword::Haste));
        assert!(zone_view_rules_annotation_labels(&mut engine, 0, permanent)
            .iter()
            .any(|label| label == "Creature types: Elemental"));
    }

    engine.state.active_player_idx = 1;
    for permanent in [artifact, enchantment] {
        let characteristics = engine
            .characteristics(permanent)
            .expect("opponent-turn characteristics");
        assert!(!characteristics.is_creature());
        assert!(!characteristics.has_keyword(Keyword::Indestructible));
        assert!(!characteristics.has_keyword(Keyword::Haste));
        assert!(zone_view_rules_annotation_labels(&mut engine, 0, permanent).is_empty());
    }
}

#[test]
fn bello_respects_layer_four_scope_changes_that_precede_its_effect() {
    let mut engine = setup(12_460_002);
    put(&mut engine, 0, "Bello, Bard of the Brambles", BELLO);
    let artifact = put(&mut engine, 0, "Gilded Lotus", "gilded_lotus");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(artifact),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Land],
            creature_types: Vec::new(),
            land_types: Vec::new(),
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: u64::MAX,
    });

    let same_evaluation = engine
        .characteristics(artifact)
        .expect("layered characteristics");
    assert!(same_evaluation.has_type("Land"));
    assert_eq!(
        (same_evaluation.power, same_evaluation.toughness),
        (None, None),
        "the type-changing effect is applied before Bello because it changes Bello's layer-4 scope"
    );
    assert!(!same_evaluation.has_keyword(Keyword::Indestructible));
    assert!(!same_evaluation.has_keyword(Keyword::Haste));

    engine.state.active_player_idx = 1;
    let next_evaluation = engine
        .characteristics(artifact)
        .expect("opponent-turn characteristics");
    assert_eq!(
        (next_evaluation.power, next_evaluation.toughness),
        (None, None)
    );
    assert!(!next_evaluation.has_keyword(Keyword::Indestructible));
    assert!(!next_evaluation.has_keyword(Keyword::Haste));
}

#[test]
fn bello_damage_trigger_resolves_after_bello_and_recipient_leave() {
    let mut engine = setup(12_460_003);
    let bello = put(&mut engine, 0, "Bello, Bard of the Brambles", BELLO);
    let artifact = put(&mut engine, 0, "Gilded Lotus", "gilded_lotus");
    let hand_before = engine.state.players[0].hand.len();

    deal_unblocked_combat_damage(&mut engine, artifact);
    assert_eq!(engine.state.stack.len(), 1, "one damage trigger is stacked");

    move_to_hand(&mut engine, 0, "Bello, Bard of the Brambles");
    move_to_hand(&mut engine, 0, "Gilded Lotus");
    assert_eq!(engine.state.objects[&bello].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Hand);
    let after_leaves = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes the stacked trigger");
    engine
        .apply_scenario_command(1, &pass())
        .expect("damage trigger resolves after both sources leave");
    assert_eq!(engine.state.stack.len(), 0);
    assert_eq!(
        engine.state.players[0].hand.len(),
        after_leaves + 1,
        "the already-triggered draw resolves independently of its source and granting static"
    );
    assert!(after_leaves >= hand_before);
}
