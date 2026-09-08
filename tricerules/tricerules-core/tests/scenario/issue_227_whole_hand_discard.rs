use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

const HEARTH: &str = "hearth_elemental_stoke_genius";

fn game() -> GameEngine {
    let mut e = GameEngine::new(
        227,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &[HEARTH]),
            deck_with("island", &["counterspell"]),
        ]),
        true,
    )
    .expect("Hearth Elemental is fully supported");
    advance_to_main1_from_game_start(&mut e);
    // Keep the draw pile populated, but choose the exact hand independently of the shuffle.
    for player in &mut e.state.players {
        for oid in std::mem::take(&mut player.hand) {
            e.state.objects.get_mut(&oid).unwrap().zone = Zone::Library;
            player.library.push_back(oid);
        }
    }
    e
}

fn cast_stoke(e: &mut GameEngine) -> u32 {
    let oid = inject_card_into_hand(e, 0, HEARTH);
    give_mana(
        e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, HEARTH);
    e.apply_command(0, &cast_spell_face(slot, vec![], 1))
        .expect("cast Stoke Genius");
    oid
}

#[test]
fn issue_227_stoke_discards_the_entire_hand_without_a_selection_then_draws() {
    for size in [0, 1, 7, 300] {
        let mut e = game();
        let discarded: Vec<_> = (0..size)
            .map(|_| inject_card_into_hand(&mut e, 0, "mountain"))
            .collect();
        let expected_draws: Vec<_> = e.state.players[0].library.iter().take(2).copied().collect();
        let adventure = cast_stoke(&mut e);
        e.apply_command(0, &pass()).unwrap();
        let batch = e.apply_command(1, &pass()).unwrap();
        assert!(
            e.state.pending_resolution.is_none(),
            "no choice for a forced complete hand"
        );
        assert!(!batch.events.iter().any(|event| matches!(
            event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
        assert_eq!(e.state.players[0].hand, expected_draws, "hand size {size}");
        assert!(discarded
            .iter()
            .all(|oid| e.state.objects[oid].zone == Zone::Graveyard));
        assert_eq!(e.state.objects[&adventure].zone, Zone::Exile);
    }
}

#[test]
fn issue_227_library_replacements_finish_before_the_two_draws_and_adventure_exit() {
    let mut e = game();
    inject_permanent_on_battlefield(&mut e, 0, "library_of_leng");
    let first = inject_card_into_hand(&mut e, 0, "grizzly_bears");
    let second = inject_card_into_hand(&mut e, 0, "mountain");
    let discarded = inject_card_into_hand(&mut e, 0, "opt");
    let adventure = cast_stoke(&mut e);
    pass_both_players(&mut e);
    let before = format!("{:?}", e.state);
    assert!(e
        .apply_command(1, &submit_resolution_choice(vec![1]))
        .is_err());
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![99]))
        .is_err());
    assert_eq!(format!("{:?}", e.state), before);
    e.apply_command(0, &submit_resolution_choice(vec![1]))
        .unwrap();
    e.apply_command(0, &submit_resolution_choice(vec![1]))
        .unwrap();
    e.apply_command(0, &submit_resolution_choice(vec![0]))
        .unwrap();
    assert_eq!(e.state.players[0].hand, [first, second, discarded]);
    assert_eq!(e.state.objects[&adventure].zone, Zone::Stack);
    let before_order = format!("{:?}", e.state);
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![first, first]))
        .is_err());
    assert_eq!(format!("{:?}", e.state), before_order);
    e.apply_command(0, &submit_resolution_choice(vec![second, first]))
        .unwrap();
    assert_eq!(
        e.state.players[0].hand,
        [second, first],
        "draw the chosen top-first order"
    );
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(e.state.zone_change_generation[&first], 2);
    assert_eq!(e.state.zone_change_generation[&second], 2);
    assert_eq!(e.state.objects[&adventure].zone, Zone::Exile);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_227_stale_whole_hand_replacement_rejects_without_partial_discard() {
    let mut e = game();
    inject_permanent_on_battlefield(&mut e, 0, "library_of_leng");
    let first = inject_card_into_hand(&mut e, 0, "mountain");
    let stale = inject_card_into_hand(&mut e, 0, "grizzly_bears");
    cast_stoke(&mut e);
    pass_both_players(&mut e);
    e.apply_command(0, &submit_resolution_choice(vec![0]))
        .unwrap();
    // Simulate the same OID leaving and returning while its original incarnation is retained.
    *e.state.zone_change_generation.entry(stale).or_default() += 2;
    let before = format!("{:?}", e.state);
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![0]))
        .is_err());
    assert_eq!(format!("{:?}", e.state), before);
    assert_eq!(e.state.players[0].hand, [first, stale]);
}

#[test]
fn issue_227_multiple_madness_triggers_wait_until_stoke_finishes() {
    let mut e = game();
    let first = inject_card_into_hand(&mut e, 0, "fiery_temper");
    let second = inject_card_into_hand(&mut e, 0, "fiery_temper");
    let adventure = cast_stoke(&mut e);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[0].hand.len(), 2);
    assert_eq!(e.state.objects[&first].zone, Zone::Exile);
    assert_eq!(e.state.objects[&second].zone, Zone::Exile);
    assert_eq!(e.state.objects[&adventure].zone, Zone::Exile);
    assert!(
        e.state.pending_resolution.is_none(),
        "madness casting is not inside Stoke resolution"
    );
    assert!(e.state.pending_trigger_order.is_some());
    while let Some(order) = e.state.pending_trigger_order.as_ref() {
        let source = order.candidates[0].object_id;
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitTriggerOrder(rv1::SubmitTriggerOrder {
                    trigger_object_id: source,
                })),
            },
        )
        .unwrap();
    }
    for _ in 0..2 {
        pass_both_players(&mut e);
        assert!(e.state.pending_resolution.is_some());
        e.apply_command(
            0,
            &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
        )
        .unwrap();
    }
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn issue_227_hearth_cost_counts_the_graveyard_union_once_and_preserves_red() {
    for count in 0..=7u32 {
        let mut e = game();
        for i in 0..count {
            inject_graveyard_card(
                &mut e,
                0,
                ["opt", "divination", "bonecrusher_giant_stomp"][i as usize % 3],
            );
        }
        inject_graveyard_card(&mut e, 0, "mountain");
        inject_graveyard_card(&mut e, 0, "grizzly_bears");
        for _ in 0..5 {
            inject_graveyard_card(&mut e, 1, "opt");
        }
        let oid = inject_card_into_hand(&mut e, 0, HEARTH);
        let slot = hand_index_for_card(&e, 0, HEARTH);
        let generic = 5u32.saturating_sub(count);
        e.state.players[0].mana_pool.colorless = generic + 1;
        assert!(
            e.apply_command(0, &cast_spell(slot, vec![])).is_err(),
            "red is never reduced"
        );
        e.state.players[0].mana_pool.red = 1;
        e.state.players[0].mana_pool.colorless = generic.saturating_sub(1);
        if generic > 0 {
            assert!(
                e.apply_command(0, &cast_spell(slot, vec![])).is_err(),
                "one mana short at {count} qualifying cards"
            );
        }
        e.state.players[0].mana_pool.colorless = generic;
        e.apply_command(0, &cast_spell(slot, vec![])).unwrap();
        assert_eq!(e.state.players[0].mana_pool.red, 0);
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
    }
}

#[test]
fn issue_227_resolved_adventure_can_cast_hearth_at_its_reduced_cost() {
    let mut e = game();
    for card in [
        "opt",
        "divination",
        "bonecrusher_giant_stomp",
        "opt",
        "divination",
    ] {
        inject_card_into_hand(&mut e, 0, card);
    }
    let oid = cast_stoke(&mut e);
    pass_both_players(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let batch = e.initial_response_batch();
    let action = batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|a| a.object_id == oid)
        .unwrap();
    assert_eq!(action.face_index, 0);
    assert_eq!(action.card_name, "Hearth Elemental");
    assert_eq!(action.cost, "{5}{R}");
    assert_eq!(action.generic_cost_reduction, 5);
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(rv1::CastSpell {
            cast_method: rv1::CastMethod::Normal as i32,
            source: Some(exile_cast_source(oid, e.state.zone_change_generation[&oid])),
            casting_permission_id: action.casting_permission_id,
            ..Default::default()
        })),
    };
    e.apply_command(0, &command).unwrap();
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
    assert!(e
        .state
        .active_exile_play_permissions
        .iter()
        .all(|p| p.object_id != oid));
}

#[test]
fn issue_227_countered_stoke_neither_discards_nor_grants_exile_permission() {
    let mut e = game();
    let retained = inject_card_into_hand(&mut e, 0, "grizzly_bears");
    let oid = cast_stoke(&mut e);
    e.apply_command(0, &pass()).unwrap();
    inject_card_into_hand(&mut e, 1, "counterspell");
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(1, &cast_spell(slot, target_object(oid)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand, [retained]);
    assert_eq!(e.state.objects[&oid].zone, Zone::Graveyard);
    assert!(e
        .state
        .active_exile_play_permissions
        .iter()
        .all(|p| p.object_id != oid));
}
