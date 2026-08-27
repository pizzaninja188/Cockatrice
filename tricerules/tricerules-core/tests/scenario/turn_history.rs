use super::helpers::*;

use tricerules_proto::ruled::v1::TargetRef;

fn issue_172_engine(cards: &[&str]) -> GameEngine {
    let mut e = GameEngine::new(
        172020,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", cards), island_only_deck()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    e
}

fn issue_172_cast(e: &mut GameEngine, card: &str, mana: ManaGift) -> RuledEventBatch {
    ensure_in_hand(e, 0, card);
    give_mana(e, 0, mana);
    e.apply_command(0, &cast_spell(hand_index_for_card(e, 0, card), vec![]))
        .unwrap()
}

#[test]
fn issue_172_late_watchers_count_prior_spending_without_retroactive_triggers() {
    for (watcher, mana, expected) in [
        (
            "wandertale_mentor",
            ManaGift {
                r: 1,
                g: 1,
                ..Default::default()
            },
            true,
        ),
        (
            "teapot_slinger",
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            false,
        ),
    ] {
        let mut e = issue_172_engine(&[watcher, "hill_giant"]);
        issue_172_cast(&mut e, watcher, mana);
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, watcher);
        assert_eq!(
            e.state.players[1].life, 20,
            "a watcher cannot trigger from its own casting"
        );
        let batch = issue_172_cast(
            &mut e,
            "hill_giant",
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        );
        assert_eq!(
            batch
                .events
                .iter()
                .any(|event| matches!(&event.ev, Some(Ev::StackPushed(s)) if s.is_triggered)),
            expected
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&source].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
            u32::from(expected)
        );
        assert_eq!(e.state.players[1].life, 20);
    }
}

#[test]
fn issue_172_sacrificed_watcher_triggers_at_mana_payment_before_leaving() {
    let mut e = issue_172_engine(&["divination", "village_rites"]);
    let source = inject_creature_on_battlefield(&mut e, 0, "teapot_slinger");
    issue_172_cast(
        &mut e,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut e);
    ensure_in_hand(&mut e, 0, "village_rites");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let command = cast_spell_with_costs(
        hand_index_for_card(&e, 0, "village_rites"),
        vec![],
        vec![CostSelection {
            cost_index: 0,
            selection: Some(
                tricerules_proto::ruled::v1::cost_selection::Selection::PermanentId(source),
            ),
        }],
    );
    e.apply_command(0, &command).unwrap();
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    let trigger = e.state.stack.last().unwrap();
    assert!(trigger.is_triggered);
    assert_eq!(trigger.source_permanent_id, Some(source));
    assert!(trigger.source_zone_change < e.state.zone_change_generation[&source]);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 18,
        "damage survives the source's sacrifice"
    );
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        4
    );
}

#[test]
fn issue_172_source_blink_does_not_apply_old_counter_to_new_incarnation() {
    use tricerules_proto::ruled::v1::{dev_command::Dev, DevCommand, DevMoveCard, DevZone};
    let mut e = issue_172_engine(&["hill_giant", "hill_giant"]);
    e.enable_dev_commands();
    let mentor = inject_creature_on_battlefield(&mut e, 0, "wandertale_mentor");
    issue_172_cast(
        &mut e,
        "hill_giant",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let source_generation = e.state.stack.last().unwrap().source_zone_change;
    for zone in [DevZone::Exile, DevZone::Battlefield] {
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: "Wandertale Mentor".into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
    }
    assert!(e.state.zone_change_generation[&mentor] > source_generation);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&mentor].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        0
    );
    issue_172_cast(
        &mut e,
        "hill_giant",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&mentor].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        0,
        "blink does not reset player spending"
    );
}

#[test]
fn issue_172_countered_spells_keep_spending_and_spell_copies_add_none() {
    let mut e = issue_172_engine(&["divination", "twincast"]);
    let mentor = inject_creature_on_battlefield(&mut e, 0, "wandertale_mentor");
    issue_172_cast(
        &mut e,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let original = e.state.stack.last().unwrap().id;
    ensure_in_hand(&mut e, 0, "twincast");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &cast_spell(
            hand_index_for_card(&e, 0, "twincast"),
            vec![TargetRef {
                object_id: original,
                ..Default::default()
            }],
        ),
    )
    .unwrap();
    pass_both_players(&mut e); // Expend's counter, above Twincast.
    pass_both_players(&mut e); // Copy Divination; no new targets to choose.
    assert!(e.state.stack.iter().any(|item| item.is_copy));
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        5
    );
    assert_eq!(
        e.state.objects[&mentor].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );

    inject_card_into_hand(&mut e, 1, "cancel");
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(
        1,
        &cast_spell(
            hand_index_for_card(&e, 1, "cancel"),
            vec![TargetRef {
                object_id: original,
                ..Default::default()
            }],
        ),
    )
    .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        5
    );
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(1)
            .mana_spent_casting_spells,
        3
    );
    assert_eq!(
        e.state.objects[&original].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn issue_172_kicker_is_actual_spell_spending_and_activations_are_not() {
    let mut e = issue_172_engine(&["grow_from_the_ashes"]);
    let mentor = inject_creature_on_battlefield(&mut e, 0, "wandertale_mentor");
    e.apply_command(0, &activate_ability(mentor, 0, vec![]))
        .unwrap();
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        0
    );
    assert_eq!(
        e.state.players[0].mana_pool.red, 1,
        "the first authored mana option is red"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut e, 0, "grow_from_the_ashes");
    e.apply_command(
        0,
        &cast_spell_with_cast_cost_groups(
            hand_index_for_card(&e, 0, "grow_from_the_ashes"),
            vec![],
            vec![CastCostGroupSelection {
                group_index: 0,
                option_index: 0,
                ..Default::default()
            }],
        ),
    )
    .unwrap();
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        5
    );
    assert!(e.state.stack.last().unwrap().is_triggered);
}

#[test]
fn issue_172_rejected_casts_do_not_change_accepted_command_replay() {
    use prost::Message;
    fn run(reject: bool) -> (Vec<RuledEventBatch>, tricerules_core::state::TurnHistory) {
        let mut e = issue_172_engine(&["hill_giant"]);
        inject_creature_on_battlefield(&mut e, 0, "wandertale_mentor");
        ensure_in_hand(&mut e, 0, "hill_giant");
        let bytes = cast_spell(hand_index_for_card(&e, 0, "hill_giant"), vec![]).encode_to_vec();
        let command = RuledCommand::decode(bytes.as_slice()).unwrap();
        if reject {
            assert!(e.apply_command(0, &command).is_err());
        }
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        );
        let mut batches = vec![e.apply_command(0, &command).unwrap()];
        while !e.state.stack.is_empty() {
            let player = e.state.priority_player_id();
            batches.push(e.apply_command(player, &pass()).unwrap());
        }
        (batches, e.state.turn_history.clone())
    }
    assert_eq!(run(false), run(true));
}

#[test]
fn issue_172_cleanup_priority_retains_spending_until_the_new_turn_instance() {
    let mut e = issue_172_engine(&["divination", "life_goes_on", "hill_giant"]);
    let boxer = inject_creature_on_battlefield(&mut e, 0, "bark-knuckle_boxer");
    let mentor = inject_creature_on_battlefield(&mut e, 0, "wandertale_mentor");
    issue_172_cast(
        &mut e,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut e);
    issue_172_cast(
        &mut e,
        "life_goes_on",
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    // Fixture for waiting triggers in a CR 514.3 cleanup priority window. The engine currently
    // rejects new casts during cleanup, so that separate casting limitation is not exercised.
    e.state.turn_step = tricerules_core::TurnStep::Cleanup;
    e.state.cleanup_priority_active = true;
    let turn = e.state.turn_instance;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.turn_instance, turn);
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        4
    );
    assert!(e
        .characteristics(boxer)
        .unwrap()
        .keywords
        .contains(&tricerules_cards::Keyword::Indestructible));
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_instance, turn + 1);
    assert_eq!(
        e.state
            .turn_history
            .previous
            .player(0)
            .mana_spent_casting_spells,
        4
    );
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        0
    );
    assert!(!e
        .characteristics(boxer)
        .unwrap()
        .keywords
        .contains(&tricerules_cards::Keyword::Indestructible));
    assert_eq!(
        e.state.objects[&mentor].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );
    advance_to_main1_from_game_start(&mut e);
    end_active_turn(&mut e, 1);
    advance_to_main1_from_game_start(&mut e);
    issue_172_cast(
        &mut e,
        "hill_giant",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&mentor].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        2
    );
}

#[test]
fn issue_172_three_seats_track_spending_independently_and_damage_each_opponent() {
    let mut e = issue_172_engine(&[]);
    // Session creation is still two-seat-only; exercise the engine's player-set contract directly.
    e.state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    inject_creature_on_battlefield(&mut e, 0, "teapot_slinger");
    for player in [1, 2, 0] {
        inject_card_into_hand(&mut e, player, "angels_mercy");
        give_mana(
            &mut e,
            player as i32,
            ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
        );
        e.state.priority_idx = player;
        let command = cast_spell(hand_index_for_card(&e, player, "angels_mercy"), vec![]);
        e.apply_command(player as i32, &command).unwrap();
        for _ in 0..12 {
            if e.state.stack.is_empty() {
                break;
            }
            e.apply_command(e.state.priority_player_id(), &pass())
                .unwrap();
        }
        assert!(e.state.stack.is_empty());
        assert_eq!(
            e.state
                .turn_history
                .current
                .player(player as i32)
                .mana_spent_casting_spells,
            4
        );
    }
    assert_eq!(
        e.state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        [27, 25, 25]
    );
}

#[test]
fn issue_172_control_change_uses_new_controllers_spending_not_an_object_cap() {
    let mut e = issue_172_engine(&["angels_mercy"]);
    let source = inject_creature_on_battlefield(&mut e, 0, "teapot_slinger");
    issue_172_cast(
        &mut e,
        "angels_mercy",
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    resolve_entire_stack_two_player(&mut e);
    let generation = e
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    e.state.players[0].battlefield.retain(|oid| *oid != source);
    e.state.players[1].battlefield.push(source);
    let object = e.state.objects.get_mut(&source).unwrap();
    object.base_controller = 1;
    object.controller = 1;
    inject_card_into_hand(&mut e, 1, "angels_mercy");
    give_mana(
        &mut e,
        1,
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(
        1,
        &cast_spell(hand_index_for_card(&e, 1, "angels_mercy"), vec![]),
    )
    .unwrap();
    let trigger = e.state.stack.last().unwrap();
    assert!(trigger.is_triggered);
    assert_eq!(trigger.controller, 1);
    assert_eq!(trigger.source_zone_change, generation);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!((e.state.players[0].life, e.state.players[1].life), (25, 25));
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        4
    );
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(1)
            .mana_spent_casting_spells,
        4
    );
}

#[test]
fn issue_172_expend_crosses_once_and_reuses_ordinary_trigger_effects() {
    let mut e = GameEngine::new(
        172002,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "forest",
                &["grizzly_bears", "grizzly_bears", "grizzly_bears"],
            ),
            island_only_deck(),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    let boxer = inject_creature_on_battlefield(&mut e, 0, "bark-knuckle_boxer");
    let slinger = inject_creature_on_battlefield(&mut e, 0, "teapot_slinger");
    let mentor = inject_creature_on_battlefield(&mut e, 0, "wandertale_mentor");
    for cast_number in 1..=3 {
        ensure_in_hand(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        let command = cast_spell(hand_index_for_card(&e, 0, "grizzly_bears"), vec![]);
        e.apply_command(0, &command).unwrap();
        assert_eq!(e.state.pending_trigger_order.is_some(), cast_number == 2);
        resolve_entire_stack_two_player(&mut e);
        let triggered = cast_number >= 2;
        assert_eq!(
            e.characteristics(boxer)
                .unwrap()
                .keywords
                .contains(&tricerules_cards::Keyword::Indestructible),
            triggered
        );
        assert_eq!(e.state.players[1].life, if triggered { 18 } else { 20 });
        assert_eq!(
            e.state.objects[&mentor].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
            u32::from(triggered)
        );
        assert!(e
            .characteristics(slinger)
            .unwrap()
            .keywords
            .contains(&tricerules_cards::Keyword::Menace));
        let snapshot = e.initial_response_batch();
        let view = snapshot
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ZoneView(view)) => Some(view),
                _ => None,
            })
            .unwrap();
        let objects = &view.per_player[0].battlefield_objects;
        let published_boxer = objects
            .iter()
            .find(|object| object.object_id == boxer)
            .unwrap();
        let published_mentor = objects
            .iter()
            .find(|object| object.object_id == mentor)
            .unwrap();
        assert_eq!(
            published_boxer
                .keywords
                .iter()
                .any(|keyword| keyword == "Indestructible"),
            triggered
        );
        assert_eq!(
            published_boxer
                .rules_annotation_labels
                .iter()
                .any(|label| label == "Indestructible"),
            triggered
        );
        assert_eq!(
            published_mentor.counters_annotation.contains("+1/+1"),
            triggered
        );
    }
}

#[test]
fn issue_172_only_successful_casts_record_actual_mana_spending() {
    let mut e = GameEngine::new(
        172001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["grizzly_bears"]),
            island_only_deck(),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    let command = cast_spell(hand_index_for_card(&e, 0, "grizzly_bears"), vec![]);
    let revision = e.state.command_index;
    assert!(e.apply_command(0, &command).is_err());
    assert_eq!(e.state.command_index, revision);
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        0
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        0
    );
    e.apply_command(0, &command).unwrap();
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        2
    );
    assert_eq!(
        e.state
            .turn_history
            .current
            .player(1)
            .mana_spent_casting_spells,
        0
    );
    assert_eq!(e.state.players[0].mana_pool.colorless, 3);
}

#[test]
fn life_goes_on_gains_eight_after_a_creature_dies() {
    let decks = Some(vec![
        deck_with("forest", &["life_goes_on", "murder"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(6101, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut e, 0, "murder");
    ensure_in_hand(&mut e, 0, "life_goes_on");
    let bear = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast Murder");
    resolve_entire_stack_two_player(&mut e);

    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let life_goes_on = hand_index_for_card(&e, 0, "life_goes_on");
    e.apply_command(0, &cast_spell(life_goes_on, vec![]))
        .expect("cast Life Goes On");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.players[0].life, 28);
    assert_eq!(e.state.turn_history.current.creatures_died, 1);
    assert_eq!(e.state.turn_history.current.spells_cast, 2);
    assert_eq!(e.state.turn_history.current.player(0).spells_cast, 2);
    assert_eq!(e.state.turn_history.current.player(1).spells_cast, 0);
}

#[test]
fn life_goes_on_gains_four_when_no_creature_died() {
    let decks = Some(vec![
        deck_with("forest", &["life_goes_on"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6102, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut e, 0, "life_goes_on");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let life_goes_on = hand_index_for_card(&e, 0, "life_goes_on");
    e.apply_command(0, &cast_spell(life_goes_on, vec![]))
        .expect("cast Life Goes On");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.players[0].life, 24);
    assert_eq!(e.state.turn_history.current.creatures_died, 0);
}

#[test]
fn conditional_amount_is_evaluated_when_the_effect_resolves() {
    let decks = Some(vec![
        deck_with("forest", &["life_goes_on"]),
        deck_with("swamp", &["murder", "grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(6103, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut e, 0, "life_goes_on");
    ensure_in_hand(&mut e, 1, "murder");
    let bear = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );

    let life_goes_on = hand_index_for_card(&e, 0, "life_goes_on");
    e.apply_command(0, &cast_spell(life_goes_on, vec![]))
        .expect("cast Life Goes On before any creature has died");
    e.apply_command(0, &pass()).expect("pass priority");
    let murder = hand_index_for_card(&e, 1, "murder");
    e.apply_command(
        1,
        &cast_spell(
            murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("respond with Murder");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.players[0].life, 28);
    assert_eq!(e.state.turn_history.current.creatures_died, 1);
}

#[test]
fn the_same_creature_can_die_more_than_once_in_a_turn() {
    let decks = Some(vec![
        deck_with("swamp", &["murder", "reanimate", "murder"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(6104, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    ensure_in_hand(&mut e, 0, "murder");
    ensure_in_hand(&mut e, 0, "reanimate");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 5,
            c: 2,
            ..Default::default()
        },
    );

    let first_murder = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            first_murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast first Murder");
    resolve_entire_stack_two_player(&mut e);

    let reanimate = hand_index_for_card(&e, 0, "reanimate");
    e.apply_command(
        0,
        &cast_spell(
            reanimate,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("reanimate the Bear");
    resolve_entire_stack_two_player(&mut e);

    ensure_in_hand(&mut e, 0, "murder");
    let second_murder = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            second_murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast second Murder");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.turn_history.current.creatures_died, 2);
}

#[test]
fn noncreature_deaths_do_not_increment_the_creature_count() {
    let decks = Some(vec![
        deck_with("mountain", &["shatterstorm", "short_sword"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6105, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "short_sword", false);
    ensure_in_hand(&mut e, 0, "shatterstorm");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );

    let shatterstorm = hand_index_for_card(&e, 0, "shatterstorm");
    e.apply_command(0, &cast_spell(shatterstorm, vec![]))
        .expect("cast Shatterstorm");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.turn_history.current.creatures_died, 0);
}

#[test]
fn cleanup_rolls_current_history_to_previous_and_resets_current() {
    let decks = Some(vec![
        deck_with("swamp", &["murder"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(6106, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "murder");
    let bear = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(
            murder,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.turn_history.current.creatures_died, 1);
    assert_eq!(e.state.turn_history.current.spells_cast, 1);

    end_active_turn(&mut e, 0);

    assert_eq!(e.state.turn_history.current.creatures_died, 0);
    assert_eq!(e.state.turn_history.current.spells_cast, 0);
    assert_eq!(e.state.turn_history.previous.creatures_died, 1);
    assert_eq!(e.state.turn_history.previous.spells_cast, 1);
    assert_eq!(e.state.turn_history.previous.player(0).spells_cast, 1);
    assert_eq!(e.state.turn_history.current.player(0).spells_cast, 0);
}

#[test]
fn rejected_casts_do_not_enter_turn_history() {
    let decks = Some(vec![
        deck_with("forest", &["life_goes_on"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6107, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut e, 0, "life_goes_on");
    let life_goes_on = hand_index_for_card(&e, 0, "life_goes_on");

    e.apply_command(0, &cast_spell(life_goes_on, vec![]))
        .expect_err("casting without green mana is rejected");

    assert_eq!(e.state.turn_history.current.spells_cast, 0);
    assert_eq!(e.state.turn_history.current.player(0).spells_cast, 0);
    assert_eq!(hand_index_for_card(&e, 0, "life_goes_on"), life_goes_on);
}
