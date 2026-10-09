use crate::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, TurnStep};
use tricerules_proto::ruled::v1::{dev_command::Dev, DevCommand, DevMoveCard, DevZone};

fn cursed_mirror_engine() -> GameEngine {
    let mut first_deck = vec!["island".to_string(); 18];
    first_deck.push("cursed_mirror".to_string());
    first_deck.push("clone".to_string());
    let decks = Some(vec![first_deck, vec!["island".to_string(); 20]]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        303_701,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine with Cursed Mirror");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cursed_mirror_reanimation_engine() -> GameEngine {
    let deck = vec!["island".to_string(); 20];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        303_702,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("engine for the physical graveyard Cursed Mirror");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn animate_native_cursed_mirror_with_tough_cookie(engine: &mut GameEngine, mirror: u32) {
    let cookie = inject_creature_on_battlefield(engine, 0, "tough_cookie");
    grant_pool(engine, 0);
    apply_ability(engine, 0, cookie, 0, target_object(mirror))
        .expect("Tough Cookie targets the noncreature artifact Cursed Mirror");
    resolve_entire_stack_two_player(engine);
    assert!(engine.characteristics(mirror).unwrap().is_creature());
    assert!(
        engine.state.objects[&mirror].copiable_values.is_none(),
        "Tough Cookie's animation changes characteristics without changing copiable values"
    );
}

fn select_resolution_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn move_named_card_to_zone(engine: &mut GameEngine, player: i32, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the selected card through a logged dev command");
}

fn advance_to_next_turn_instance(engine: &mut GameEngine) {
    let _ = advance_to_next_turn_instance_capturing_batches(engine);
}

fn advance_to_next_turn_instance_capturing_batches(
    engine: &mut GameEngine,
) -> Vec<RuledEventBatch> {
    let starting_turn = engine.state.turn_instance;
    let mut batches = Vec::new();
    for _ in 0..40 {
        if engine.state.turn_instance > starting_turn {
            return batches;
        }
        let actor = engine.state.priority_player_id();
        let command = if engine.state.cleanup_discard_player == Some(actor) {
            let index = engine.state.player_idx(actor).unwrap();
            discard_cleanup((engine.state.players[index].hand.len() - 1) as u32)
        } else {
            pass()
        };
        batches.push(
            engine
                .apply_command(actor, &command)
                .expect("pass through the end of the turn"),
        );
    }
    panic!("game did not advance to the next turn instance");
}

fn advance_to_cleanup_priority(engine: &mut GameEngine) -> Vec<RuledEventBatch> {
    let mut batches = Vec::new();
    for _ in 0..40 {
        if engine.state.turn_step == TurnStep::Cleanup && engine.state.cleanup_priority_active {
            return batches;
        }
        let actor = engine.state.priority_player_id();
        let command = if engine.state.cleanup_discard_player == Some(actor) {
            let index = engine.state.player_idx(actor).unwrap();
            discard_cleanup((engine.state.players[index].hand.len() - 1) as u32)
        } else {
            pass()
        };
        batches.push(
            engine
                .apply_command(actor, &command)
                .expect("pass through the turn until cleanup"),
        );
    }
    panic!("cleanup did not grant priority after its state-based action");
}

fn mirror_battlefield_view(
    batch: &RuledEventBatch,
    mirror: u32,
) -> &tricerules_proto::ruled::v1::BattlefieldObject {
    batch
        .events
        .iter()
        .rev()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| player.battlefield_objects.iter())
                .find(|object| object.object_id == mirror),
            _ => None,
        })
        .expect("zone view contains Cursed Mirror")
}

#[test]
fn cursed_mirror_temporarily_copies_a_creature_with_haste_then_restores_its_mana_ability() {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get("cursed_mirror").is_some(),
        "Cursed Mirror must have a complete rules definition"
    );

    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&mirror].zone,
        tricerules_core::Zone::Stack
    );

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cursed Mirror copy-source choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(pending.presentation.candidates.contains(&target));
    let deciding_player = pending.deciding_player;
    let copied_batch = engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![target]))
        .expect("choose an opponent's creature");

    let copied = engine
        .characteristics(mirror)
        .expect("copied Cursed Mirror");
    assert_eq!(
        copied.names,
        ["Grizzly Bears"],
        "object={:?}; pending={:?}",
        engine.state.objects[&mirror],
        engine.state.pending_resolution
    );
    assert!(copied.is_creature());
    assert!(!copied.has_type("Artifact"));
    assert!(engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    let copied_view = mirror_battlefield_view(&copied_batch, mirror);
    assert_eq!(copied_view.effective_display_name, "Grizzly Bears");
    assert!(copied_view
        .keywords
        .iter()
        .any(|keyword| keyword == "Haste"));
    assert!(copied_view.activated_abilities.is_empty());

    let cleanup_batches = advance_to_next_turn_instance_capturing_batches(&mut engine);
    let restored = engine
        .characteristics(mirror)
        .expect("restored Cursed Mirror");
    assert_eq!(restored.names, ["Cursed Mirror"]);
    assert!(!restored.is_creature());
    assert!(restored.has_type("Artifact"));
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    assert!(engine.state.objects[&mirror].copiable_values.is_none());
    let cleanup_view = cleanup_batches
        .iter()
        .rev()
        .find_map(|batch| {
            batch.events.iter().rev().find_map(|event| match &event.ev {
                Some(Ev::ZoneView(view)) => view
                    .per_player
                    .iter()
                    .flat_map(|player| player.battlefield_objects.iter())
                    .find(|object| object.object_id == mirror),
                _ => None,
            })
        })
        .expect("cleanup emits a public battlefield view for the reverted Mirror");
    assert_eq!(cleanup_view.effective_display_name, "Cursed Mirror");
    assert_eq!(cleanup_view.activated_abilities.len(), 1);
}

#[test]
fn later_clone_entry_copy_replaces_mirror_haste_and_is_not_reverted_at_cleanup() {
    let registry = tricerules_cards::registry::global();
    assert!(registry.get("cursed_mirror").is_some());

    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let clone = inject_creature_on_battlefield(&mut engine, 1, "clone");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&mirror].zone,
        tricerules_core::Zone::Stack
    );

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cursed Mirror copy-source choice");
    assert!(pending.presentation.candidates.contains(&clone));
    let deciding_player = pending.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![clone]))
        .expect("copy Clone first");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copied Clone supplies a later entry-copy choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(pending.presentation.candidates.contains(&bear));
    let deciding_player = pending.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![bear]))
        .expect("let Clone's later copy effect replace Cursed Mirror's copy");

    let copied = engine.characteristics(mirror).expect("final copied Mirror");
    assert_eq!(
        copied.names,
        ["Grizzly Bears"],
        "object={:?}; pending={:?}",
        engine.state.objects[&mirror],
        engine.state.pending_resolution
    );
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    let copy_revision = engine.state.objects[&mirror].copy_revision;

    advance_to_next_turn_instance(&mut engine);
    let after_cleanup = engine
        .characteristics(mirror)
        .expect("persistent Clone copy");
    assert_eq!(after_cleanup.names, ["Grizzly Bears"]);
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    assert_eq!(engine.state.objects[&mirror].copy_revision, copy_revision);
}

#[test]
fn mirror_copying_animated_mirror_chains_temporary_copy_and_expires_to_native_values() {
    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let animated_mirror = inject_permanent_on_battlefield(&mut engine, 0, "cursed_mirror");
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    animate_native_cursed_mirror_with_tough_cookie(&mut engine, animated_mirror);
    grant_pool(&mut engine, 0);

    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);

    let first_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the entering Mirror can copy the animated Mirror");
    assert!(first_choice
        .presentation
        .candidates
        .contains(&animated_mirror));
    let deciding_player = first_choice.deciding_player;
    engine
        .apply_command(
            deciding_player,
            &submit_resolution_choice(vec![animated_mirror]),
        )
        .expect("temporarily copy the animated Cursed Mirror");

    let acquired_mirror_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copy acquires Cursed Mirror's as-enters replacement");
    assert!(acquired_mirror_choice
        .presentation
        .candidates
        .contains(&bears));
    let deciding_player = acquired_mirror_choice.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![bears]))
        .expect("the acquired Cursed Mirror replacement temporarily copies Bears");
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Grizzly Bears"]
    );
    assert!(engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));

    advance_to_next_turn_instance(&mut engine);

    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Cursed Mirror"]
    );
    assert!(engine.state.objects[&mirror].copiable_values.is_none());
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
}

#[test]
fn mirror_clone_animated_mirror_chain_restores_the_intervening_persistent_copy() {
    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let clone = inject_creature_on_battlefield(&mut engine, 1, "clone");
    let animated_mirror = inject_permanent_on_battlefield(&mut engine, 0, "cursed_mirror");
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    animate_native_cursed_mirror_with_tough_cookie(&mut engine, animated_mirror);
    grant_pool(&mut engine, 0);

    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);

    let mirror_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cursed Mirror's entry choice");
    assert!(mirror_choice.presentation.candidates.contains(&clone));
    let deciding_player = mirror_choice.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![clone]))
        .expect("the temporary Mirror effect copies Clone");

    let clone_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the acquired Clone replacement offers a persistent copy");
    assert!(clone_choice
        .presentation
        .candidates
        .contains(&animated_mirror));
    let deciding_player = clone_choice.deciding_player;
    engine
        .apply_command(
            deciding_player,
            &submit_resolution_choice(vec![animated_mirror]),
        )
        .expect("Clone persistently copies the animated Cursed Mirror");

    let persistent_copy = engine.state.objects[&mirror]
        .copiable_values
        .clone()
        .expect("the intermediate Clone copy is persistent");
    let persistent_occurrence = engine.state.objects[&mirror]
        .active_copy_occurrence
        .expect("the intermediate copy has a stable occurrence");
    let persistent_revision = engine.state.objects[&mirror].copy_revision;
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Cursed Mirror"]
    );
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));

    let acquired_mirror_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the persistent copy acquires Cursed Mirror's as-enters replacement");
    assert!(acquired_mirror_choice
        .presentation
        .candidates
        .contains(&bears));
    let deciding_player = acquired_mirror_choice.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![bears]))
        .expect("the acquired Cursed Mirror replacement temporarily copies Bears");
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Grizzly Bears"]
    );
    assert!(engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));

    advance_to_next_turn_instance(&mut engine);

    let restored = &engine.state.objects[&mirror];
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Cursed Mirror"]
    );
    assert!(restored.copiable_values.is_some());
    assert_eq!(restored.active_copy_occurrence, Some(persistent_occurrence));
    assert!(
        restored.copy_revision > persistent_revision,
        "cleanup reversion advances the mutation revision without changing the restored occurrence"
    );
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    assert_eq!(
        restored.copiable_values.as_ref().unwrap().source_card_id,
        persistent_copy.source_card_id
    );
}

#[test]
fn cleanup_sba_without_triggers_still_grants_cleanup_priority() {
    let registry = tricerules_cards::registry::global();
    assert!(registry.get("cursed_mirror").is_some());

    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&mirror].zone,
        tricerules_core::Zone::Stack
    );
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cursed Mirror copy-source choice")
        .deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![target]))
        .expect("copy the opponent's creature");

    let pacifism = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    let equipment = inject_permanent_on_battlefield(&mut engine, 0, "bonesplitter");
    engine.state.objects.get_mut(&pacifism).unwrap().attached_to =
        Some(AttachmentRecipient::Object(mirror));
    engine
        .state
        .objects
        .get_mut(&equipment)
        .unwrap()
        .attached_to = Some(AttachmentRecipient::Object(mirror));
    let mirror_object = engine.state.objects.get_mut(&mirror).unwrap();
    mirror_object.set_counter(CounterKind::PlusOnePlusOne, 2);
    mirror_object.tapped = true;
    let generation_before_cleanup = engine.state.zone_change_generation[&mirror];
    let cleanup_batches = advance_to_cleanup_priority(&mut engine);

    assert_eq!(engine.state.turn_step, TurnStep::Cleanup);
    assert!(engine.state.cleanup_priority_active);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(
        engine.state.objects[&pacifism].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&equipment].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert_eq!(
        engine.state.objects[&mirror].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.zone_change_generation[&mirror],
        generation_before_cleanup
    );
    assert!(engine.state.objects[&mirror].tapped);
    assert_eq!(
        engine.state.objects[&mirror].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Cursed Mirror"]
    );
    let cleanup_view = cleanup_batches
        .iter()
        .rev()
        .find_map(|batch| {
            batch.events.iter().rev().find_map(|event| match &event.ev {
                Some(Ev::ZoneView(view)) => view
                    .per_player
                    .iter()
                    .flat_map(|player| player.battlefield_objects.iter())
                    .find(|object| object.object_id == mirror),
                _ => None,
            })
        })
        .expect("cleanup emits a public battlefield view after copy expiry");
    assert_eq!(cleanup_view.effective_display_name, "Cursed Mirror");

    advance_to_next_turn_instance(&mut engine);
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Cursed Mirror"]
    );
}

#[test]
fn welder_returned_cursed_mirror_expires_its_entry_copy_at_cleanup() {
    let mut engine = cursed_mirror_reanimation_engine();
    let welder = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let mirror = inject_graveyard_card(&mut engine, 0, "cursed_mirror");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    let exchange = activate_ability_for(
        &engine,
        welder,
        0,
        vec![
            TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id: departure,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: mirror,
                group_index: 1,
                ..Default::default()
            },
        ],
    );
    engine.apply_command(0, &exchange).unwrap();
    pass_both_players(&mut engine);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Welder return invokes Cursed Mirror's as-enters choice")
        .deciding_player;
    assert_eq!(
        engine.state.objects[&mirror].zone,
        tricerules_core::Zone::Graveyard
    );
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![target]))
        .expect("choose the creature as Cursed Mirror enters from the graveyard");
    assert_eq!(
        engine.state.objects[&mirror].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Grizzly Bears"]
    );
    let first_generation = engine.state.zone_change_generation[&mirror];
    assert!(engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));

    move_named_card_to_zone(&mut engine, 0, "Cursed Mirror", DevZone::Graveyard);
    assert_eq!(
        engine.state.objects[&mirror].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.zone_change_generation[&mirror] > first_generation);
    let second_welder = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let second_departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let second_exchange = activate_ability_for(
        &engine,
        second_welder,
        0,
        vec![
            TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id: second_departure,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: mirror,
                group_index: 1,
                ..Default::default()
            },
        ],
    );
    engine.apply_command(0, &second_exchange).unwrap();
    pass_both_players(&mut engine);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the new battlefield incarnation asks its copy choice")
        .deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![target]))
        .expect("copy the creature with the new incarnation");
    let generation = engine.state.zone_change_generation[&mirror];
    assert!(generation > first_generation);

    advance_to_next_turn_instance(&mut engine);

    assert_eq!(
        engine.state.objects[&mirror].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.zone_change_generation[&mirror], generation);
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Cursed Mirror"],
        "the canonical noncast entry path must retain and expire the temporary copy"
    );
    assert!(engine.state.objects[&mirror].copiable_values.is_none());
}

#[test]
fn mirror_copied_clever_impersonator_keeps_black_vise_opponent_choice_after_cleanup() {
    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let impersonator = inject_creature_on_battlefield(&mut engine, 1, "clever_impersonator");
    engine
        .state
        .objects
        .get_mut(&impersonator)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let vise = inject_permanent_on_battlefield(&mut engine, 1, "black_vise");
    grant_pool(&mut engine, 0);
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cursed Mirror copy choice")
        .deciding_player;
    engine
        .apply_command(
            deciding_player,
            &submit_resolution_choice(vec![impersonator]),
        )
        .expect("copy Clever Impersonator");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copied Clever Impersonator offers a later copy choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(pending.presentation.candidates.contains(&vise));
    let deciding_player = pending.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![vise]))
        .expect("let the later copy effect replace the Mirror copy");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copied Black Vise chooses an opponent on entry");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let deciding_player = pending.deciding_player;
    engine
        .apply_command(deciding_player, &select_resolution_branch(0))
        .expect("choose the opponent for the acquired Black Vise ability");
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Black Vise"]
    );
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, mirror),
        ["Chosen opponent: P1"]
    );

    advance_to_next_turn_instance(&mut engine);

    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Black Vise"]
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, mirror),
        ["Chosen opponent: P1"],
        "the later copy's linked choice remains bound to its stable copy occurrence"
    );
}

#[test]
fn mirrorworks_token_copy_of_cursed_mirror_expires_while_preserving_token_identity() {
    let mut engine = cursed_mirror_engine();
    inject_permanent_on_battlefield(&mut engine, 0, "mirrorworks");
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cursed Mirror copy-source choice")
        .deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(Vec::new()))
        .expect("leave the original artifact unmodified so Mirrorworks can copy it");

    for _ in 0..8 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        pass_both_players(&mut engine);
    }
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Mirrorworks offers its optional token-copy branch")
        .deciding_player;
    engine
        .apply_command(deciding_player, &select_resolution_branch(0))
        .expect("pay for the Mirrorworks token copy");
    submit_mana_resolution_decision(
        &mut engine,
        deciding_player,
        ResolutionChoiceDecision::PayMana,
    )
    .expect("pay Mirrorworks' {2} cost");

    let token = engine
        .state
        .objects
        .iter()
        .find_map(|(&object_id, object)| {
            object
                .token_origin
                .as_ref()
                .is_some_and(|origin| origin.source_card_id == "cursed_mirror")
                .then_some(object_id)
        })
        .expect("the provisional token copy has Cursed Mirror's token origin");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the token copy uses Cursed Mirror's own as-enters choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    let deciding_player = pending.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![bear]))
        .expect("copy Grizzly Bears with the token");
    assert_eq!(
        engine.characteristics(token).unwrap().names,
        ["Grizzly Bears"]
    );
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
    assert!(engine.state.objects[&token].token_origin.is_some());

    advance_to_next_turn_instance(&mut engine);

    assert_eq!(
        engine.state.objects[&token].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&token].token_origin.is_some());
    assert_eq!(
        engine.characteristics(token).unwrap().names,
        ["Cursed Mirror"],
        "token-batch commitment must register the same temporary-copy cleanup"
    );
    assert!(engine.state.objects[&token].copiable_values.is_none());
    assert!(!engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
}

#[test]
fn clone_of_a_copying_mirror_keeps_copiable_haste_after_mirror_reverts() {
    let registry = tricerules_cards::registry::global();
    assert!(registry.get("cursed_mirror").is_some());

    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let mirror_hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(mirror_hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cursed Mirror copy-source choice")
        .deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![target]))
        .expect("copy the opponent's creature");
    assert!(engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));

    let clone = relocate_to_hand(&mut engine, 0, "clone");
    let clone_hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == clone)
        .expect("selected physical Clone in hand");
    engine
        .apply_command(0, &cast_spell(clone_hand_index, Vec::new()))
        .expect("cast Clone while Mirror is a copy");
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Clone copy-source choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(pending.presentation.candidates.contains(&mirror));
    let deciding_player = pending.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![mirror]))
        .expect("copy Mirror's copiable values");
    assert_eq!(
        engine.characteristics(clone).unwrap().names,
        ["Grizzly Bears"]
    );
    assert!(engine.effective_has_keyword(clone, tricerules_cards::Keyword::Haste));
    let clone_copy_revision = engine.state.objects[&clone].copy_revision;

    advance_to_next_turn_instance(&mut engine);
    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Cursed Mirror"]
    );
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    assert_eq!(
        engine.characteristics(clone).unwrap().names,
        ["Grizzly Bears"]
    );
    assert!(engine.effective_has_keyword(clone, tricerules_cards::Keyword::Haste));
    assert_eq!(
        engine.state.objects[&clone].copy_revision,
        clone_copy_revision
    );
}

#[test]
fn declining_cursed_mirror_copy_leaves_its_artifact_mana_ability_available() {
    let registry = tricerules_cards::registry::global();
    assert!(registry.get("cursed_mirror").is_some());

    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let _target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("optional Cursed Mirror copy-source choice")
        .deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(Vec::new()))
        .expect("decline to copy a creature");

    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Cursed Mirror"]
    );
    assert!(engine.state.objects[&mirror].copiable_values.is_none());
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    let view = engine.initial_response_batch();
    assert_eq!(
        mirror_battlefield_view(&view, mirror)
            .activated_abilities
            .len(),
        1
    );
}

#[test]
fn copied_elvish_visionary_etb_ability_triggers_for_cursed_mirrors_controller() {
    let registry = tricerules_cards::registry::global();
    assert!(registry.get("cursed_mirror").is_some());

    let mut engine = cursed_mirror_engine();
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let visionary = inject_creature_on_battlefield(&mut engine, 1, "elvish_visionary");
    grant_pool(&mut engine, 0);
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == mirror)
        .expect("selected physical Cursed Mirror in hand");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    let hand_size_before_draw = engine.state.players[0].hand.len();
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cursed Mirror copy-source choice");
    assert!(pending.presentation.candidates.contains(&visionary));
    let deciding_player = pending.deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![visionary]))
        .expect("copy Elvish Visionary");

    assert_eq!(
        engine.characteristics(mirror).unwrap().names,
        ["Elvish Visionary"]
    );
    assert!(engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_size_before_draw + 1
    );
}
