use super::helpers::*;

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: TargetRefKind::Stack as i32,
    }]
}

fn valid_stack_targets(e: &mut GameEngine, player: i32, hand_index: usize) -> Vec<u32> {
    e.initial_response_batch().legal_by_player[&player].valid_targets_by_hand_slot
        [&((hand_index as u32) << 8)]
        .groups[0]
        .valid_stack_ids
        .clone()
}

#[test]
fn issue_205_exact_and_minimum_filters_publish_and_enforce_the_same_targets() {
    let decks = Some(vec![
        vec![
            "hill_giant".into(),
            "grizzly_bears".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "spell_snare".into(),
            "disdainful_stroke".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(205_001, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 6,
            g: 2,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 6,
            ..Default::default()
        },
    );

    let hill = hand_index_for_card(&e, 0, "hill_giant");
    e.apply_command(0, &cast_spell(hill, vec![])).unwrap();
    let hill_spell = e.state.stack.last().unwrap().id;
    e.apply_command(0, &pass()).unwrap();

    let snare = hand_index_for_card(&e, 1, "spell_snare");
    let stroke = hand_index_for_card(&e, 1, "disdainful_stroke");
    assert!(!valid_stack_targets(&mut e, 1, snare).contains(&hill_spell));
    assert!(valid_stack_targets(&mut e, 1, stroke).contains(&hill_spell));

    let hand_before = e.state.players[1].hand.clone();
    let mana_before = e.state.players[1].mana_pool;
    let stack_before = e.state.stack.len();
    let revision_before = e.state.command_index;
    assert!(e
        .apply_command(1, &cast_spell(snare, stack_target(hill_spell)))
        .is_err());
    assert_eq!(e.state.players[1].hand, hand_before);
    assert_eq!(e.state.players[1].mana_pool, mana_before);
    assert_eq!(e.state.stack.len(), stack_before);
    assert_eq!(e.state.command_index, revision_before);

    let stroke = hand_index_for_card(&e, 1, "disdainful_stroke");
    e.apply_command(1, &cast_spell(stroke, stack_target(hill_spell)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0]
            .graveyard
            .iter()
            .filter(|&&oid| e.state.objects[&oid].card_id == "hill_giant")
            .count(),
        1
    );

    let bear = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(bear, vec![])).unwrap();
    let bear_spell = e.state.stack.last().unwrap().id;
    e.apply_command(0, &pass()).unwrap();
    let snare = hand_index_for_card(&e, 1, "spell_snare");
    assert_eq!(valid_stack_targets(&mut e, 1, snare), vec![bear_spell]);
    e.apply_command(1, &cast_spell(snare, stack_target(bear_spell)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0]
            .graveyard
            .iter()
            .filter(|&&oid| e.state.objects[&oid].card_id == "grizzly_bears")
            .count(),
        1
    );
}

#[test]
fn issue_205_resolution_rejects_a_stale_stack_target_generation() {
    let decks = Some(vec![
        vec![
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "spell_snare".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(205_002, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let bear = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(bear, vec![])).unwrap();
    let bear_spell = e.state.stack.last().unwrap().id;
    e.apply_command(0, &pass()).unwrap();
    let snare = hand_index_for_card(&e, 1, "spell_snare");
    e.apply_command(1, &cast_spell(snare, stack_target(bear_spell)))
        .unwrap();

    *e.state
        .zone_change_generation
        .entry(bear_spell)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.players[0].battlefield.contains(&bear_spell));
}
