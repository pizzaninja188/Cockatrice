use super::helpers::*;
use tricerules_core::TurnStep;
use tricerules_proto::ruled::v1::{ruled_event::Ev, ChoiceKind, RuledEventBatch};

fn mana_state(engine: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &engine.state.players[player].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn batch_ability_flag(
    batch: &RuledEventBatch,
    player_id: i32,
    object_id: u32,
) -> Option<(bool, bool)> {
    batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .find(|player| player.player_id == player_id)
                .and_then(|player| {
                    player
                        .battlefield_objects
                        .iter()
                        .find(|object| object.object_id == object_id)
                })
                .and_then(|object| {
                    object
                        .activated_abilities
                        .first()
                        .map(|ability| (view.battlefields_unchanged, ability.activatable))
                }),
            _ => None,
        })
}

#[test]
fn successful_activation_refreshes_public_legality_and_rejects_stale_replay() {
    let mut engine = anthem_engine(10_201, "mountain");
    let devotee = inject_creature_on_battlefield(&mut engine, 0, "temur_devotee");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let _ = engine.initial_response_batch();

    let command = activate_ability(devotee, 0, vec![]);
    let batch = engine
        .apply_command(0, &command)
        .expect("the first activation is legal");

    assert!(
        engine.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );
    assert_eq!(
        batch_ability_flag(&batch, 0, devotee),
        Some((false, false)),
        "the activation-use change invalidates the battlefield cache and greys the ability"
    );
    let key = (devotee as u64) << 32;
    assert!(
        !batch.legal_by_player[&0].cost_choices_by_ability[&key].non_mana_costs_payable,
        "the same legality predicate disables cost collection"
    );

    let mana_before = mana_state(&engine, 0);
    let uses_before = engine.state.activation_uses_this_turn.clone();
    engine
        .apply_command(0, &command)
        .expect_err("the stale second activation must be rejected");
    assert_eq!(mana_state(&engine, 0), mana_before);
    assert_eq!(engine.state.activation_uses_this_turn, uses_before);
}

#[test]
fn failed_payment_does_not_consume_the_activation() {
    let mut engine = anthem_engine(10_202, "mountain");
    let devotee = inject_creature_on_battlefield(&mut engine, 0, "temur_devotee");

    engine
        .apply_command(0, &activate_ability(devotee, 0, vec![]))
        .expect_err("the generic cost is unaffordable");
    assert!(engine.state.activation_uses_this_turn.is_empty());
    assert_eq!(zone_view_ability_flags(&mut engine, 0, devotee), [true]);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            r: 1,
            ..Default::default()
        },
    );
    let mut command = activate_ability(devotee, 0, vec![]);
    let mut activation = match command.cmd.as_ref().unwrap() {
        Cmd::ActivateAbility(activation) => activation.clone(),
        _ => unreachable!(),
    };
    let preview = engine.preview_payment(
        0,
        &tricerules_proto::ruled::v1::PreviewPayment {
            transaction_id: 1,
            revision: 1,
            activate_ability: Some(activation.clone()),
            ..Default::default()
        },
    );
    assert!(preview.valid, "{}", preview.error);
    activation.payment = preview.selection;
    activation.payment.as_mut().unwrap().mana = Some(tricerules_proto::ruled::v1::PaymentMana {
        r: 1,
        ..Default::default()
    });
    command.cmd = Some(Cmd::ActivateAbility(activation));
    engine
        .apply_command(0, &command)
        .expect("the unconsumed activation remains available");
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(zone_view_ability_flags(&mut engine, 0, devotee), [false]);
}

#[test]
fn control_and_face_changes_preserve_the_limit_but_zone_changes_reset_identity() {
    let mut engine = anthem_engine(10_203, "mountain");
    let devotee = inject_creature_on_battlefield(&mut engine, 0, "temur_devotee");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(devotee, 0, vec![]))
        .expect("first controller activates");

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != devotee);
    engine.state.players[1].battlefield.push(devotee);
    let object = engine.state.objects.get_mut(&devotee).expect("devotee");
    object.base_controller = 1;
    object.controller = 1;
    engine.state.priority_idx = 1;
    assert_eq!(zone_view_ability_flags(&mut engine, 1, devotee), [false]);
    engine
        .apply_command(1, &activate_ability(devotee, 0, vec![]))
        .expect_err("changing control does not evade once-per-turn");

    *engine
        .state
        .zone_change_generation
        .entry(devotee)
        .or_insert(0) += 1;
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 1, devotee), [true]);
    apply_ability(&mut engine, 1, devotee, 0, vec![])
        .expect("leave-and-return creates a new object identity");

    *engine
        .state
        .face_change_generation
        .entry(devotee)
        .or_insert(0) += 1;
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 1, devotee), [false]);
    apply_ability(&mut engine, 1, devotee, 0, vec![])
        .expect_err("a face-status change preserves the authored ability identity");
}

#[test]
fn activation_resets_only_after_cleanup_really_advances_the_turn() {
    let mut engine = anthem_engine(10_204, "mountain");
    let devotee = inject_creature_on_battlefield(&mut engine, 0, "temur_devotee");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(devotee, 0, vec![]))
        .expect("activate in the current turn");

    let gnomes = inject_creature_on_battlefield(&mut engine, 0, "bottle_gnomes");
    engine
        .apply_command(0, &activate_ability(gnomes, 0, vec![]))
        .expect("put an ability on the stack for cleanup priority");
    engine.state.turn_step = TurnStep::Cleanup;
    engine.state.cleanup_priority_active = true;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;

    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.cleanup_priority_active);
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, devotee),
        [false],
        "resolving the cleanup stack does not end the turn"
    );

    pass_both_players(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert!(engine.state.activation_uses_this_turn.is_empty());
    assert_eq!(zone_view_ability_flags(&mut engine, 0, devotee), [true]);
}

#[test]
fn mardu_devotee_scry_resolves_before_its_limited_mana_ability_is_used() {
    let mut engine = anthem_engine(10_205, "mardu_devotee");
    inject_library_card(&mut engine, 0, "forest");
    inject_library_card(&mut engine, 0, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "mardu_devotee");
    engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Mardu Devotee");
    pass_both_players(&mut engine);

    let choice = (0..6)
        .find_map(|_| {
            let player = engine.state.priority_player_id();
            let batch = engine
                .apply_command(player, &pass())
                .expect("pass toward the enters-trigger resolution");
            find_resolution_choice(&batch)
        })
        .expect("scry 2 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids.len(), 2);
    engine
        .apply_command(
            0,
            &submit_resolution_choice(choice.candidate_object_ids.clone()),
        )
        .expect("put both seen cards on the bottom");
    assert!(engine.state.pending_resolution.is_none());

    let devotee = *engine.state.players[0]
        .battlefield
        .iter()
        .find(|object_id| engine.state.objects[object_id].card_id == "mardu_devotee")
        .expect("Mardu Devotee on the battlefield");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, devotee, 0, vec![])
        .expect("activate after the scry continuation finishes");
    assert_eq!(zone_view_ability_flags(&mut engine, 0, devotee), [false]);
}

#[test]
fn same_seed_and_commands_replay_the_same_activation_state() {
    fn run() -> GameEngine {
        let mut engine = anthem_engine(10_206, "mountain");
        let devotee = inject_creature_on_battlefield(&mut engine, 0, "sultai_devotee");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        engine
            .apply_command(0, &activate_ability(devotee, 0, vec![]))
            .expect("accepted command");
        engine
    }

    let first = run();
    let second = run();
    assert_eq!(
        first.state.activation_uses_this_turn,
        second.state.activation_uses_this_turn
    );
    assert_eq!(mana_state(&first, 0), mana_state(&second, 0));
}
