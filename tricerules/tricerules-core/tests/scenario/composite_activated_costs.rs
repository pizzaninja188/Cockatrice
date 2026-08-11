use crate::helpers::*;

#[test]
fn apparatus_pays_mana_tap_and_self_sacrifice_before_resolution() {
    let decks = Some(vec![
        deck_with("mountain", &["explosive_apparatus"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(5201, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let apparatus = relocate_to_battlefield(&mut e, 0, "explosive_apparatus", false);
    e.state.players[0].mana_pool.colorless = 3;

    e.apply_command(0, &activate_ability(apparatus, 0, target_player(1)))
        .expect("pay all three cost components");
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        e.state.objects[&apparatus].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(e.state.stack.len(), 1, "ability remains on the stack");

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 18);
}

#[test]
fn portcullis_vine_can_tap_then_sacrifice_itself() {
    let decks = Some(vec![
        deck_with("forest", &["portcullis_vine"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(5202, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let vine = relocate_to_battlefield(&mut e, 0, "portcullis_vine", false);
    e.state.players[0].mana_pool.colorless = 2;
    let hand_before = e.state.players[0].hand.len();

    e.apply_command(
        0,
        &activate_ability_with_costs(vine, 0, vec![], vec![permanent_cost_selection(2, vine)]),
    )
    .expect("the tapped source remains a legal sacrifice selection");
    assert_eq!(
        e.state.objects[&vine].zone,
        tricerules_core::Zone::Graveyard
    );

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
}

#[test]
fn invalid_filtered_sacrifice_rolls_back_mana_and_tap() {
    let decks = Some(vec![
        deck_with("forest", &["portcullis_vine"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(5203, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let vine = relocate_to_battlefield(&mut e, 0, "portcullis_vine", false);
    let forest = relocate_to_battlefield(&mut e, 0, "forest", false);
    e.state.players[0].mana_pool.colorless = 2;

    e.apply_command(
        0,
        &activate_ability_with_costs(vine, 0, vec![], vec![permanent_cost_selection(2, forest)]),
    )
    .expect_err("a noncreature without defender cannot pay the sacrifice cost");

    assert_eq!(e.state.players[0].mana_pool.colorless, 2);
    assert!(!e.state.objects[&vine].tapped);
    assert_eq!(
        e.state.objects[&vine].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        e.state.objects[&forest].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn discard_cost_uses_authoritative_hand_slot() {
    let decks = Some(vec![
        deck_with("forest", &["noose_constrictor"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(5204, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let constrictor = relocate_to_battlefield(&mut e, 0, "noose_constrictor", false);
    let discarded = e.state.players[0].hand[0];

    e.apply_command(
        0,
        &activate_ability_with_costs(constrictor, 0, vec![], vec![hand_cost_selection(0, 0)]),
    )
    .expect("discard the selected physical hand object");

    assert_eq!(
        e.state.objects[&discarded].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(e.state.stack.len(), 1);
}
