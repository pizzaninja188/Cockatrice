//! Issue #329 — Warp and Flashback alternative cast-method costs on generated faces.
//!
//! These scenarios drive the generated `warp_cost` / `flashback_cost` definitions through the
//! authoritative command path. CR 702.185 governs Warp's hand alternative cost and its delayed
//! end-step exile plus owner recast permission; CR 702.34 governs Flashback's graveyard
//! alternative cost and exile-on-resolution; CR 601.2 / 202.3 govern the printed-cost normal cast.

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};

fn engine(seed: u64) -> GameEngine {
    let mut e = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![vec!["forest".into(); 30]; 2]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut e);
    e
}

/// Every published hand cast action for `card_name` as `(cast_method, cost)`.
fn hand_cast_costs(e: &mut GameEngine, card_name: &str) -> Vec<(i32, String)> {
    e.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .filter(|action| {
            action.kind == tricerules_proto::ruled::v1::HandActionKind::HandActionCastSpell as i32
                && action.card_name == card_name
        })
        .map(|action| (action.cast_method, action.cost.clone()))
        .collect()
}

#[test]
fn issue_329_bygone_colossus_warp_pays_the_alternative_cost_and_exiles_then_recasts() {
    let mut e = engine(329_001);
    let colossus = inject_card_into_hand(&mut e, 0, "bygone_colossus");
    grant_pool(&mut e, 0);
    let costs = hand_cast_costs(&mut e, "Bygone Colossus");
    assert!(
        costs.contains(&(CastMethod::Normal as i32, "{9}".into())),
        "the printed mana cost stays available: {costs:?}"
    );
    assert!(
        costs.contains(&(CastMethod::Warp as i32, "{3}".into())),
        "the warp alternative cost is published: {costs:?}"
    );

    let slot = hand_index_for_card(&e, 0, "bygone_colossus");
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                source: Some(hand_cast_source(slot)),
                cast_method: CastMethod::Warp as i32,
                ..Default::default()
            })),
        },
    )
    .expect("cast at the warp cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&colossus].zone, Zone::Battlefield);

    // CR 702.185: the delayed trigger exiles it at the beginning of the next end step.
    e.state.turn_step = TurnStep::Main2;
    pass_both_players(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&colossus].zone, Zone::Exile);
    let generation = e.state.zone_change_generation[&colossus];
    let permission_id = e
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.object_id == colossus)
        .expect("warp recast permission")
        .group_id;

    // The owner may cast it from exile on a later turn at its printed cost.
    e.state.turn_instance += 1;
    e.state.turn_step = TurnStep::Main1;
    e.state.priority_idx = 0;
    grant_pool(&mut e, 0);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                source: Some(exile_cast_source(colossus, generation)),
                cast_method: CastMethod::Normal as i32,
                casting_permission_id: Some(permission_id),
                ..Default::default()
            })),
        },
    )
    .expect("recast the warped creature from exile");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&colossus].zone, Zone::Battlefield);
    assert!(e.state.active_exile_play_permissions.is_empty());
}

#[test]
fn issue_329_think_twice_normal_cast_then_flashback_exiles_on_resolution() {
    let mut e = engine(329_002);
    let twice = inject_card_into_hand(&mut e, 0, "think_twice");
    grant_pool(&mut e, 0);
    let costs = hand_cast_costs(&mut e, "Think Twice");
    assert!(
        costs.contains(&(CastMethod::Normal as i32, "{1}{U}".into())),
        "a normal cast still uses the printed cost: {costs:?}"
    );

    let slot = hand_index_for_card(&e, 0, "think_twice");
    e.apply_command(0, &cast_spell(slot, vec![]))
        .expect("normal cast from hand");
    let hand_before = e.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&twice].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "Draw a card resolves on the normal cast"
    );

    // CR 702.34: cast from the graveyard for the flashback cost, then exile on resolution.
    grant_pool(&mut e, 0);
    let generation = e.state.zone_change_generation[&twice];
    let batch = e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    source: Some(graveyard_cast_source(twice, generation)),
                    cast_method: CastMethod::Flashback as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("flashback cast from the graveyard");
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackPushed(pushed)) if pushed.ability_annotation == "Flashback"
        )
    }));

    let hand_before = e.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.players[0].exile.contains(&twice),
        "CR 702.34a: a spell cast with flashback is exiled, not buried"
    );
    assert!(!e.state.players[0].graveyard.contains(&twice));
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "Draw a card resolves on the flashback cast"
    );
}
