use crate::helpers::*;

fn grouped(object_ids: &[u32]) -> Vec<TargetRef> {
    object_ids
        .iter()
        .copied()
        .map(|object_id| TargetRef {
            object_id,
            damage_amount: 0,
            group_index: 0,
            kind: 0,
        })
        .collect()
}

#[test]
fn ghostform_accepts_zero_and_applies_to_two_targets() {
    let decks = Some(vec![
        deck_with(
            "island",
            &["ghostform", "ghostform", "grizzly_bears", "grizzly_bears"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(73_101, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "ghostform");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            ..Default::default()
        },
    );

    let zero = hand_index_for_card(&engine, 0, "ghostform");
    engine
        .apply_command(0, &cast_spell(zero, vec![]))
        .expect("cast Ghostform with zero targets");
    resolve_entire_stack_two_player(&mut engine);
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, first).is_empty());

    ensure_in_hand(&mut engine, 0, "ghostform");
    let two = hand_index_for_card(&engine, 0, "ghostform");
    engine
        .apply_command(0, &cast_spell(two, grouped(&[first, second])))
        .expect("cast Ghostform with two targets");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, first),
        vec!["Can't be blocked"]
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, second),
        vec!["Can't be blocked"]
    );
}

#[test]
fn soul_salvage_returns_zero_one_or_two_creatures_in_selection_order() {
    for (seed, count) in [(73_201, 0usize), (73_202, 1), (73_203, 2)] {
        let decks = Some(vec![
            deck_with("swamp", &["soul_salvage"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
        advance_to_main1_from_game_start(&mut engine);
        let first = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
        let second = inject_graveyard_card(&mut engine, 0, "storm_crow");
        ensure_in_hand(&mut engine, 0, "soul_salvage");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                b: 3,
                ..Default::default()
            },
        );
        let spell = hand_index_for_card(&engine, 0, "soul_salvage");
        let selected = [second, first].into_iter().take(count).collect::<Vec<_>>();
        engine
            .apply_command(0, &cast_spell(spell, grouped(&selected)))
            .expect("cast Soul Salvage");
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(
            engine.state.objects[&first].zone,
            if count == 2 {
                tricerules_core::Zone::Hand
            } else {
                tricerules_core::Zone::Graveyard
            }
        );
        assert_eq!(
            engine.state.objects[&second].zone,
            if count >= 1 {
                tricerules_core::Zone::Hand
            } else {
                tricerules_core::Zone::Graveyard
            }
        );
        if count == 2 {
            let hand = &engine.state.players[0].hand;
            let second_pos = hand
                .iter()
                .position(|oid| *oid == second)
                .expect("second in hand");
            let first_pos = hand
                .iter()
                .position(|oid| *oid == first)
                .expect("first in hand");
            assert!(second_pos < first_pos, "selection order is deterministic");
        }
    }
}

#[test]
fn grouped_submission_rejects_duplicates_overfill_and_wrong_zone_before_costs() {
    let decks = Some(vec![
        deck_with("island", &["frost_breath"]),
        deck_with(
            "forest",
            &["grizzly_bears", "grizzly_bears", "grizzly_bears"],
        ),
    ]);
    let mut engine = GameEngine::new(73_301, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let third = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "frost_breath");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "frost_breath");
    let hand_before = engine.state.players[0].hand.clone();
    let mana_before = engine.state.players[0].mana_pool.blue;

    for invalid in [grouped(&[first, first]), grouped(&[first, second, third])] {
        assert!(engine
            .apply_command(0, &cast_spell(spell, invalid))
            .is_err());
        assert_eq!(engine.state.players[0].hand, hand_before);
        assert_eq!(engine.state.players[0].mana_pool.blue, mana_before);
    }

    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != first);
    engine.state.players[1].graveyard.push(first);
    engine.state.objects.get_mut(&first).expect("first").zone = tricerules_core::Zone::Graveyard;
    assert!(engine
        .apply_command(0, &cast_spell(spell, grouped(&[first])))
        .is_err());
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(engine.state.players[0].mana_pool.blue, mana_before);
}

#[test]
fn frost_breath_partially_resolves_when_one_target_leaves() {
    let decks = Some(vec![
        deck_with("island", &["frost_breath"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(73_302, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let surviving = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let stale = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "frost_breath");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "frost_breath");
    let mut targets = grouped(&[surviving, stale]);
    for target in &mut targets {
        target.kind = TargetRefKind::Permanent as i32;
    }
    let cast_batch = engine
        .apply_command(0, &cast_spell(spell, targets))
        .expect("cast Frost Breath");
    let pushed = cast_batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::StackPushed(push)) => Some(push),
            _ => None,
        });
    let pushed = pushed.expect("Frost Breath is pushed");
    assert_eq!(pushed.targets.len(), 2);
    assert!(pushed
        .targets
        .iter()
        .all(|target| target.kind == TargetRefKind::Permanent as i32));
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != stale);
    engine.state.players[1].hand.push(stale);
    engine.state.objects.get_mut(&stale).expect("stale").zone = tricerules_core::Zone::Hand;

    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&surviving].tapped);
    assert!(!engine.state.objects[&stale].tapped);
    assert_eq!(engine.state.skip_next_untap.len(), 1);
}

#[test]
fn frost_breath_fizzles_when_every_chosen_target_is_illegal() {
    let decks = Some(vec![
        deck_with("island", &["frost_breath"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(73_303, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "frost_breath");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "frost_breath");
    engine
        .apply_command(0, &cast_spell(spell, grouped(&[first, second])))
        .expect("cast Frost Breath");
    for stale in [first, second] {
        engine.state.players[1]
            .battlefield
            .retain(|oid| *oid != stale);
        engine.state.players[1].hand.push(stale);
        engine.state.objects.get_mut(&stale).expect("stale").zone = tricerules_core::Zone::Hand;
    }

    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects[&first].tapped);
    assert!(!engine.state.objects[&second].tapped);
    assert!(engine.state.skip_next_untap.is_empty());
}
