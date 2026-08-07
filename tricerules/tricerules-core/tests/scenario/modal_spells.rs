use super::helpers::*;
use tricerules_cards::Keyword;

fn modal_decks(card_id: &str) -> Option<Vec<Vec<String>>> {
    Some(vec![
        vec![
            card_id.into(),
            card_id.into(),
            "mountain".into(),
            "plains".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
        ],
        forest_only_deck(),
    ])
}

fn fund_boros(e: &mut GameEngine) {
    give_mana(
        e,
        0,
        ManaGift {
            r: 1,
            w: 1,
            ..Default::default()
        },
    );
}

#[test]
fn boros_charm_damage_mode_is_atomic_and_public() {
    let mut e = GameEngine::new(19001, &[0, 1], 20, modal_decks("boros_charm"), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    fund_boros(&mut e);
    let index = hand_index_for_card(&e, 0, "boros_charm");
    let batch = e
        .apply_command(0, &cast_modal_spell(index, vec![(0, target_player(1))]))
        .unwrap();

    let pushed = batch.events.iter().find_map(|event| match &event.ev {
        Some(Ev::StackPushed(pushed)) => Some(pushed),
        _ => None,
    });
    let pushed = pushed.expect("stack push");
    assert_eq!(pushed.chosen_mode_indices, vec![0]);
    assert_eq!(
        pushed.chosen_mode_labels,
        vec!["Deal 4 damage to target player"]
    );
    assert_eq!(pushed.targets[0].object_id, 1);

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 16);
}

#[test]
fn healing_salve_life_mode_completes_the_card() {
    let mut e = GameEngine::new(19002, &[0, 1], 20, modal_decks("healing_salve"), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&e, 0, "healing_salve");
    e.apply_command(0, &cast_modal_spell(index, vec![(0, target_player(1))]))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 23);
}

#[test]
fn boros_charm_keyword_modes_apply_to_the_correct_snapshot() {
    let mut e = GameEngine::new(19003, &[0, 1], 20, modal_decks("boros_charm"), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    fund_boros(&mut e);
    let first = hand_index_for_card(&e, 0, "boros_charm");
    e.apply_command(0, &cast_modal_spell(first, vec![(1, vec![])]))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(own, Keyword::Indestructible));
    assert!(!e.effective_has_keyword(opponent, Keyword::Indestructible));

    // Cast the second copy and prove the targeted mode uses its own mode target group.
    fund_boros(&mut e);
    let second = hand_index_for_card(&e, 0, "boros_charm");
    e.apply_command(
        0,
        &cast_modal_spell(
            second,
            vec![(
                2,
                vec![TargetRef {
                    object_id: opponent,
                    damage_amount: 0,
                }],
            )],
        ),
    )
    .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(opponent, Keyword::DoubleStrike));
}

/// Issue #42 through the modal path: mode 2 ("Target creature gains double strike until end of
/// turn") is a `GrantKeywordsTarget`, so it must reject non-battlefield targets at cast time and
/// must not advertise them in the mode's own target group (CR 115.1).
#[test]
fn boros_charm_double_strike_mode_rejects_targets_outside_the_battlefield() {
    let mut e = GameEngine::new(19010, &[0, 1], 20, modal_decks("boros_charm"), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let buried = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    fund_boros(&mut e);

    let index = hand_index_for_card(&e, 0, "boros_charm");
    let batch = e.initial_response_batch();
    let legal = batch.legal_by_player.get(&0).expect("legal actions for P0");
    let action = legal
        .hand_actions
        .iter()
        .find(|a| a.hand_index == index as u32)
        .expect("Boros Charm hand action");
    let double_strike = action
        .modes
        .iter()
        .find(|m| m.mode_index == 2)
        .expect("double strike mode");
    let targets = double_strike.targets.as_ref().expect("mode target group");
    assert_eq!(targets.valid_permanent_ids, vec![creature]);
    assert!(targets.valid_graveyard_ids.is_empty());
    assert!(!targets.can_target_self && !targets.can_target_opponent);

    for (label, object_id) in [("a graveyard card", buried), ("a player", 1u32)] {
        let result = e.apply_command(
            0,
            &cast_modal_spell(
                index,
                vec![(
                    2,
                    vec![TargetRef {
                        object_id,
                        damage_amount: 0,
                    }],
                )],
            ),
        );
        assert!(result.is_err(), "{label} must not be a legal mode-2 target");
    }

    e.apply_command(
        0,
        &cast_modal_spell(
            index,
            vec![(
                2,
                vec![TargetRef {
                    object_id: creature,
                    damage_amount: 0,
                }],
            )],
        ),
    )
    .expect("a battlefield creature is still a legal mode-2 target");
}

#[test]
fn modal_cast_rejects_bad_counts_duplicates_and_legacy_targets() {
    for (seed, command) in [
        (19004, cast_modal_spell(0, vec![])),
        (
            19005,
            cast_modal_spell(0, vec![(0, target_player(1)), (0, target_player(1))]),
        ),
        (19006, cast_modal_spell(0, vec![(99, vec![])])),
        (19007, cast_spell(0, target_player(1))),
    ] {
        let mut e = GameEngine::new(seed, &[0, 1], 20, modal_decks("boros_charm"), true).unwrap();
        advance_to_main1_from_game_start(&mut e);
        fund_boros(&mut e);
        // The seven-card deck puts every card in hand, but shuffle determines the slot.
        let index = hand_index_for_card(&e, 0, "boros_charm");
        let mut command = command;
        if let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() {
            cast.source = Some(hand_cast_source(index));
        }
        assert!(e.apply_command(0, &command).is_err());
    }
}

#[test]
fn copied_modal_spell_retains_modes_and_mode_targets() {
    let decks = Some(vec![
        vec![
            "boros_charm".into(),
            "mountain".into(),
            "plains".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "twincast".into(),
            "island".into(),
            "island".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(19008, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    fund_boros(&mut e);
    let charm = hand_index_for_card(&e, 0, "boros_charm");
    e.apply_command(0, &cast_modal_spell(charm, vec![(0, target_player(1))]))
        .unwrap();
    let charm_stack_id = e.state.stack.last().unwrap().id;

    e.apply_command(0, &pass()).unwrap();
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast = hand_index_for_card(&e, 1, "twincast");
    e.apply_command(
        1,
        &cast_spell(
            twincast,
            vec![TargetRef {
                object_id: charm_stack_id,
                damage_amount: 0,
            }],
        ),
    )
    .unwrap();
    while !e.state.stack.is_empty() {
        if e.state.pending_resolution.is_some() {
            let deciding_player = e.state.pending_resolution.as_ref().unwrap().deciding_player;
            e.apply_command(deciding_player, &submit_resolution_choice(vec![1]))
                .unwrap();
        } else {
            pass_both_players(&mut e);
        }
    }

    // The retained copy and original both use damage mode targeting P1.
    assert_eq!(e.state.players[1].life, 12);
}

fn cryptic_decks() -> Option<Vec<Vec<String>>> {
    Some(vec![
        vec![
            "cryptic_command".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "grizzly_bears".into(),
            "forest".into(),
        ],
        forest_only_deck(),
    ])
}

fn fund_cryptic(e: &mut GameEngine, player: i32) {
    give_mana(
        e,
        player,
        ManaGift {
            u: 4,
            ..Default::default()
        },
    );
}

#[test]
fn cryptic_command_bounce_then_tap_uses_printed_order_and_relative_controller() {
    let mut e = GameEngine::new(19009, &[0, 1], 20, cryptic_decks(), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bounced = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let other_opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    fund_cryptic(&mut e, 0);

    let index = hand_index_for_card(&e, 0, "cryptic_command");
    e.apply_command(
        0,
        &cast_modal_spell(
            index,
            vec![
                (
                    1,
                    vec![TargetRef {
                        object_id: bounced,
                        damage_amount: 0,
                    }],
                ),
                (2, vec![]),
            ],
        ),
    )
    .unwrap();
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&bounced].zone, tricerules_core::Zone::Hand);
    assert!(e.state.objects[&other_opponent].tapped);
    assert!(!e.state.objects[&own].tapped);
}

#[test]
fn cryptic_command_counter_and_draw_resolve_together() {
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
            "cryptic_command".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(19010, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let bear_index = hand_index_for_card(&e, 0, "grizzly_bears");
    let bear_oid = e.state.players[0].hand[bear_index];
    e.apply_command(0, &cast_spell(bear_index, vec![])).unwrap();
    let bear_stack_id = e.state.stack.last().unwrap().id;
    e.apply_command(0, &pass()).unwrap();

    fund_cryptic(&mut e, 1);
    let hand_before = e.state.players[1].hand.len();
    let cryptic_index = hand_index_for_card(&e, 1, "cryptic_command");
    e.apply_command(
        1,
        &cast_modal_spell(
            cryptic_index,
            vec![
                (
                    0,
                    vec![TargetRef {
                        object_id: bear_stack_id,
                        damage_amount: 0,
                    }],
                ),
                (3, vec![]),
            ],
        ),
    )
    .unwrap();
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&bear_oid].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(e.state.players[1].hand.len(), hand_before);
}

#[test]
fn cryptic_command_requires_exactly_two_distinct_modes() {
    let mut e = GameEngine::new(19011, &[0, 1], 20, cryptic_decks(), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    fund_cryptic(&mut e, 0);
    let permanent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let index = hand_index_for_card(&e, 0, "cryptic_command");

    assert!(e
        .apply_command(0, &cast_modal_spell(index, vec![(3, vec![])]))
        .is_err());
    assert!(e
        .apply_command(
            0,
            &cast_modal_spell(
                index,
                vec![
                    (
                        1,
                        vec![TargetRef {
                            object_id: permanent,
                            damage_amount: 0,
                        }],
                    ),
                    (2, vec![]),
                    (3, vec![]),
                ],
            ),
        )
        .is_err());
}
