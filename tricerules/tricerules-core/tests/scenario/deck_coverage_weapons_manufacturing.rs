//! Actual-card coverage for Weapons Manufacturing and its Munitions token.
//!
//! Oracle and the empty Scryfall rulings response were checked on 2026-10-06. CR 603.6a covers
//! entry triggers, 603.10a covers leaves-the-battlefield triggers, 111.7 preserves a token's
//! departure trigger until it resolves, and 115.4 limits "any target" to creature, player,
//! planeswalker, or battle targets.

use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget, TargetRef, TargetRefKind,
};

const WEAPONS_MANUFACTURING: &str = "weapons_manufacturing";
const MUNITIONS: &str = "munitions";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "grizzly_bears",
                "sol_ring",
                "mind_stone",
                "disenchant",
                "farewell",
            ],
        ),
        deck_with("forest", &["sol_ring"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn choose_trigger_target(kind: TargetRefKind, object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                kind: kind as i32,
                object_id,
                group_index: 0,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

fn resolve_top_spell(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let mut batch = Default::default();
    for _ in 0..engine.state.players.len() {
        let priority = engine.state.priority_player_id();
        batch = engine
            .apply_command(priority, &pass())
            .expect("pass priority to resolve the spell");
    }
    batch
}

#[test]
fn weapons_manufacturing_and_munitions_have_the_complete_printed_definitions() {
    let registry = CardRegistry::global();
    let card = registry
        .get(WEAPONS_MANUFACTURING)
        .expect("Weapons Manufacturing registry definition");
    assert_eq!(card.name, "Weapons Manufacturing");
    assert_eq!(card.primary_face().face_id.as_str(), WEAPONS_MANUFACTURING);
    assert_eq!(card.primary_face().mana_cost.to_string(), "{1}{R}");
    assert_eq!(card.primary_face().types, vec!["Enchantment"]);
    assert_eq!(card.primary_face().triggered_abilities.len(), 1);

    let token = registry.get(MUNITIONS).expect("Munitions token definition");
    assert!(registry.is_token(MUNITIONS));
    assert_eq!(token.name, "Munitions");
    assert_eq!(token.primary_face().types, vec!["Artifact"]);
    assert_eq!(token.primary_face().triggered_abilities.len(), 1);
}

#[test]
fn own_nontoken_artifacts_create_munitions_and_its_any_target_leaves_trigger_survives() {
    let mut engine = engine(202_610_601);
    let card = inject_card_into_hand(&mut engine, 0, WEAPONS_MANUFACTURING);
    let slot = hand_index_for_card(&engine, 0, WEAPONS_MANUFACTURING);
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Weapons Manufacturing");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&card].zone, Zone::Battlefield);

    move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    assert!(battlefield_token_oids(&engine, 0, MUNITIONS).is_empty());

    move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    assert!(battlefield_token_oids(&engine, 0, MUNITIONS).is_empty());

    let sol_ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    resolve_entire_stack_two_player(&mut engine);
    let first_token = battlefield_token_oids(&engine, 0, MUNITIONS)
        .into_iter()
        .next()
        .expect("own nontoken artifact creates Munitions");
    let first_characteristics = engine
        .characteristics(first_token)
        .expect("token characteristics");
    assert!(first_characteristics.has_name("Munitions"));
    assert_eq!(first_characteristics.types, vec!["Artifact"]);
    assert!(first_characteristics.colors.is_empty());

    move_ready_to_battlefield(&mut engine, 0, "mind_stone");
    resolve_entire_stack_two_player(&mut engine);
    let tokens = battlefield_token_oids(&engine, 0, MUNITIONS);
    assert_eq!(
        tokens.len(),
        2,
        "the Munitions artifact token does not recurse"
    );

    inject_card_into_hand(&mut engine, 0, "disenchant");
    let disenchant_slot = hand_index_for_card(&engine, 0, "disenchant");
    engine
        .apply_command(0, &cast_spell(disenchant_slot, target_object(first_token)))
        .expect("Disenchant targets Munitions");
    let destroyed_token_batch = resolve_top_spell(&mut engine);
    assert!(!engine.state.objects.contains_key(&first_token));
    let destroy_prompt = destroyed_token_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::TriggerNeedsTarget(prompt)) => Some(prompt),
            _ => None,
        })
        .expect("leaves-the-battlefield trigger asks for any target");
    assert_eq!(destroy_prompt.source_permanent_id, first_token);
    assert_eq!(destroy_prompt.controller_player_id, 0);

    let before_illegal_target = format!("{:?}", engine.state);
    assert!(
        engine
            .apply_command(
                0,
                &choose_trigger_target(TargetRefKind::Permanent, sol_ring),
            )
            .is_err(),
        "a noncreature artifact is not an any-target object"
    );
    assert_eq!(format!("{:?}", engine.state), before_illegal_target);

    engine
        .apply_command(0, &choose_trigger_target(TargetRefKind::Player, 1))
        .expect("choose the opponent as the any target");
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(first_token),
        "the stack retains the exact departed token source"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 18);

    let second_token = battlefield_token_oids(&engine, 0, MUNITIONS)[0];
    inject_card_into_hand(&mut engine, 0, "farewell");
    let farewell_slot = hand_index_for_card(&engine, 0, "farewell");
    engine
        .apply_command(0, &cast_modal_spell(farewell_slot, vec![(0, Vec::new())]))
        .expect("cast Farewell's artifact mode");
    let exiled_token_batch = resolve_top_spell(&mut engine);
    assert!(!engine.state.objects.contains_key(&second_token));
    let exile_prompt = exiled_token_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::TriggerNeedsTarget(prompt)) => Some(prompt),
            _ => None,
        })
        .expect("leaves-the-battlefield trigger also sees exile");
    assert_eq!(exile_prompt.source_permanent_id, second_token);
    assert!(engine.state.players[0].battlefield.contains(&card));

    engine
        .apply_command(0, &choose_trigger_target(TargetRefKind::Player, 1))
        .expect("choose opponent after the token is exiled");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 16);
}
