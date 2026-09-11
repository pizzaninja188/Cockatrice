//! Issue #255 — exact tap-sacrifice land draw recipe using the shared activation transaction.
//!
//! Oracle and rulings checked 2026-09-11. CR 602.1–.2 defines activated abilities and
//! their payment process, CR 601.2h and 118.3 require complete payment, CR 701.21
//! defines sacrifice, CR 701.26 defines tapping, and CR 121 governs drawing.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

#[test]
fn generated_land_draw_activation_is_generation_safe_atomic_and_resolves_from_the_stack() {
    let decks = Some(vec![
        deck_with("forest", &["airship_engine_room"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(255_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "airship_engine_room", false);
    inject_library_card(&mut engine, 0, "forest");
    let hand_before = engine.state.players[0].hand.len();
    let ability_key = (u64::from(source) << 32) | 1;
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .cost_choices_by_ability
        .contains_key(&ability_key));

    engine.state.priority_idx = 1;
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .expect_err("the controller cannot activate without priority");
    engine.state.priority_idx = 0;

    let unfunded = activate_ability_for(&engine, source, 1, vec![]);
    engine
        .apply_command(0, &unfunded)
        .expect_err("insufficient mana must reject the entire activation");
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let mut stale = activate_ability_for(&engine, source, 1, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    ability.expected_zone_change_generation += 1;
    engine
        .apply_command(0, &stale)
        .expect_err("stale source generation must reject before payment");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 4);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());

    apply_ability(&mut engine, 0, source, 1, vec![]).expect("activate generated draw ability");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "paid ability remains on the stack"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert!(!engine.initial_response_batch().legal_by_player[&0]
        .cost_choices_by_ability
        .contains_key(&ability_key));

    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
}
