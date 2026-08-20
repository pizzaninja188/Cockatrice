use crate::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_proto::ruled::v1::ruled_event::Ev;

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

    let batch = e
        .apply_command(0, &activate_ability(apparatus, 0, target_player(1)))
        .expect("pay all three cost components");
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log))
                if log.text == "P0 activates Explosive Apparatus sacrificing Explosive Apparatus: {3}, {T}, Sacrifice this artifact: It deals 2 damage to any target. — P1"
        )
    }));
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
fn hungry_ghoul_publishes_only_other_controlled_creatures_as_cost_choices() {
    let decks = Some(vec![
        deck_with("swamp", &["hungry_ghoul", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(5205, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let ghoul = relocate_to_battlefield(&mut e, 0, "hungry_ghoul", false);
    let friendly = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let opposing = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);

    let batch = e.apply_command(0, &pass()).expect("publish legal costs");
    let key = u64::from(ghoul) << 32;
    let choices = &batch.legal_by_player[&0].cost_choices_by_ability[&key];
    let sacrifice = choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 1)
        .expect("sacrifice cost choice");

    assert_eq!(sacrifice.candidate_ids, [friendly]);
    assert!(!sacrifice.candidate_ids.contains(&ghoul));
    assert!(!sacrifice.candidate_ids.contains(&opposing));
    assert!(choices.non_mana_costs_payable);
}

#[test]
fn hungry_ghoul_is_not_payable_without_another_creature() {
    let decks = Some(vec![
        deck_with("swamp", &["hungry_ghoul"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(5207, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let ghoul = relocate_to_battlefield(&mut e, 0, "hungry_ghoul", false);

    let batch = e.apply_command(0, &pass()).expect("publish legal costs");
    let key = u64::from(ghoul) << 32;
    let choices = &batch.legal_by_player[&0].cost_choices_by_ability[&key];
    let sacrifice = choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 1)
        .expect("sacrifice cost choice");

    assert!(sacrifice.candidate_ids.is_empty());
    assert!(!choices.non_mana_costs_payable);
}

#[test]
fn hungry_ghoul_rejects_its_source_atomically_then_accepts_another_creature() {
    let decks = Some(vec![
        deck_with("swamp", &["hungry_ghoul", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(5206, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let ghoul = relocate_to_battlefield(&mut e, 0, "hungry_ghoul", false);
    let other = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    e.state.players[0].mana_pool.colorless = 2;

    e.apply_command(
        0,
        &activate_ability_with_costs(ghoul, 0, vec![], vec![permanent_cost_selection(1, ghoul)]),
    )
    .expect_err("another creature cannot be the source itself");
    assert_eq!(e.state.players[0].mana_pool.colorless, 2);
    assert_eq!(
        e.state.objects[&ghoul].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        e.state.objects[&other].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(!e.state.objects[&ghoul].tapped);
    assert!(e.state.stack.is_empty());

    e.apply_command(
        0,
        &activate_ability_with_costs(ghoul, 0, vec![], vec![permanent_cost_selection(1, other)]),
    )
    .expect("another controlled creature pays the sacrifice cost");
    assert_eq!(e.state.players[0].mana_pool.colorless, 1);
    assert_eq!(
        e.state.objects[&ghoul].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        e.state.objects[&other].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(e.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&ghoul].counter_count(CounterKind::PlusOnePlusOne),
        1
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
    let discarded_slot = hand_index_for_card(&e, 0, "forest");
    let discarded = e.state.players[0].hand[discarded_slot];

    let batch = e
        .apply_command(
            0,
            &activate_ability_with_costs(
                constrictor,
                0,
                vec![],
                vec![hand_cost_selection(0, discarded_slot as u32)],
            ),
        )
        .expect("discard the selected physical hand object");
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log))
                if log.text == "P0 activates Noose Constrictor discarding Forest: Discard a card: This creature gets +1/+1 until end of turn."
        )
    }));

    assert_eq!(
        e.state.objects[&discarded].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(e.state.stack.len(), 1);
}
