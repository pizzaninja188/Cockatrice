use super::helpers::*;
use tricerules_cards::{CardRegistry, ContinuousEffectKind, ControllerReference, EffectDuration};
use tricerules_core::TurnStep;
use tricerules_core::Zone;
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

fn choose(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn parked(players: &[i32]) -> (GameEngine, u32, RuledEventBatch) {
    let mut engine = GameEngine::new(50602, players, 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "black_vise");
    give_mana(
        &mut engine,
        players[0],
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "black_vise");
    semantic::accepted(&mut engine, players[0], &cast_spell(slot, vec![]));
    let mut batch = RuledEventBatch::default();
    for _ in players {
        let actor = engine.state.priority_player_id();
        batch = engine.apply_command(actor, &pass()).unwrap();
    }
    assert!(engine.state.pending_resolution.is_some());
    (engine, source, batch)
}

fn reject_unchanged(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

fn advance_until(engine: &mut GameEngine, active: i32, step: TurnStep) {
    for _ in 0..180 {
        if engine.state.active_player_id() == active && engine.state.turn_step == step {
            return;
        }
        if engine.state.cleanup_discard_player.is_some() {
            resolve_cleanup_discards_if_any(engine);
        } else {
            pass_priority_round(engine);
        }
    }
    panic!("did not reach P{active} {step:?}");
}

// Unrelated hand contents are fixture setup. The tested cast, entry and upkeep use commands.
fn hand_size(engine: &mut GameEngine, seat: usize, size: usize) {
    for oid in std::mem::take(&mut engine.state.players[seat].hand) {
        engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Library;
        engine.state.players[seat].library.push_back(oid);
    }
    for _ in 0..size {
        inject_card_into_hand(engine, seat, "island");
    }
}

fn modifier(engine: &mut GameEngine, source: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(source),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn black_vise_rejects_stale_generation_and_copy_occurrence_atomically() {
    for copy_change in [false, true] {
        let (mut engine, source, _) = parked(&[9, 0, 23, 41]);
        if copy_change {
            engine.state.objects.get_mut(&source).unwrap().copy_revision += 1;
        } else {
            *engine
                .state
                .zone_change_generation
                .entry(source)
                .or_default() += 1;
        }
        reject_unchanged(&mut engine, 9, &choose(0));
        assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    }
}

#[test]
fn black_vise_uses_resolution_hand_size_and_triggers_even_when_zero() {
    for (before, after) in [(7, 9), (4, 4), (3, 6), (0, 0)] {
        let (mut engine, source, _) = parked(&[0, 1]);
        engine.apply_command(0, &choose(0)).unwrap();
        hand_size(&mut engine, 1, before);
        advance_until(&mut engine, 1, TurnStep::Upkeep);
        assert_eq!(engine.state.stack.len(), 1, "no threshold on the trigger");
        assert_eq!(engine.state.stack[0].source_permanent_id, Some(source));
        assert_eq!(engine.state.stack[0].controller, 0);
        assert_eq!(
            engine.state.stack[0].trigger_context.affected_player,
            Some(1)
        );
        hand_size(&mut engine, 1, after);
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.players[1].life, 20 - (after as i32 - 4).max(0));
        assert_eq!(engine.state.players[0].life, 20);
    }
}

#[test]
fn black_vise_preserves_choice_after_control_transfer_even_to_chosen_player() {
    let (mut engine, source, _) = parked(&[9, 0, 23, 41]);
    engine.apply_command(9, &choose(0)).unwrap();
    modifier(
        &mut engine,
        source,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(0),
        },
    );
    hand_size(&mut engine, 1, 8);
    advance_until(&mut engine, 0, TurnStep::Upkeep);
    assert_eq!(engine.state.objects[&source].controller, 0);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].controller, 0);
    assert_eq!(
        engine.state.stack[0].trigger_context.affected_player,
        Some(0)
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, source),
        ["Chosen opponent: P0"]
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 16);
    advance_until(&mut engine, 23, TurnStep::Upkeep);
    assert!(
        engine.state.stack.is_empty(),
        "only the designated player's upkeep qualifies"
    );
}

#[test]
fn black_vise_chosen_departure_keeps_designation_without_retargeting() {
    let (mut engine, source, _) = parked(&[9, 0, 23, 41]);
    engine.apply_command(9, &choose(0)).unwrap();
    engine.apply_command(0, &concede()).unwrap();
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, source),
        ["Chosen opponent: P0"]
    );
    advance_until(&mut engine, 23, TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    advance_until(&mut engine, 41, TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn black_vise_acquired_copy_is_undefined_and_native_choice_restores() {
    let (mut engine, source, _) = parked(&[0, 1]);
    engine.apply_command(0, &choose(0)).unwrap();
    let face = CardRegistry::global()
        .get("black_vise")
        .unwrap()
        .primary_face()
        .clone();
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: "black_vise".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Black Vise".into(),
    });
    engine.state.objects.get_mut(&source).unwrap().copy_revision += 1;
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, source).is_empty());
    advance_until(&mut engine, 1, TurnStep::Upkeep);
    assert!(
        engine.state.stack.is_empty(),
        "acquired same-definition pair cannot borrow native choice"
    );
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = None;
    engine.state.objects.get_mut(&source).unwrap().copy_revision += 1;
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, source),
        ["Chosen opponent: P1"]
    );
    advance_until(&mut engine, 0, TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    advance_until(&mut engine, 1, TurnStep::Upkeep);
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn black_vise_suppression_hides_active_link_and_restores_original_choice() {
    let (mut engine, source, _) = parked(&[0, 1]);
    engine.apply_command(0, &choose(0)).unwrap();
    modifier(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, source),
        ["Loses all abilities"]
    );
    advance_until(&mut engine, 1, TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    engine.state.continuous_effects.clear();
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, source),
        ["Chosen opponent: P1"]
    );
    advance_until(&mut engine, 0, TurnStep::Upkeep);
    advance_until(&mut engine, 1, TurnStep::Upkeep);
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn black_vise_decider_departure_abandons_pending_entry() {
    let (mut engine, source, _) = parked(&[9, 0, 23, 41]);
    engine.apply_command(9, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&source));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.diagnostic_snapshot().unwrap()["state"].get("pending_replacement_event"),
        Some(&serde_json::Value::Null)
    );
}

fn dev_move(
    engine: &mut GameEngine,
    zone: tricerules_proto::ruled::v1::DevZone,
) -> RuledEventBatch {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard};
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Black Vise".into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap()
}

#[test]
fn black_vise_queued_trigger_survives_logged_dev_bounce_and_fresh_reentry_choice() {
    use tricerules_proto::ruled::v1::DevZone;
    let (mut engine, source, _) = parked(&[0, 1, 2]);
    engine.apply_command(0, &choose(0)).unwrap();
    hand_size(&mut engine, 1, 7);
    advance_until(&mut engine, 1, TurnStep::Upkeep);
    let old_generation = engine.state.zone_change_generation[&source];
    let old_trigger = engine.state.stack[0].id;
    engine.enable_dev_commands();
    // Logged fixture moves exercise the normal entry/zone funnels while the old trigger waits.
    dev_move(&mut engine, DevZone::Hand);
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, source).is_empty());
    let batch = dev_move(&mut engine, DevZone::Battlefield);
    assert!(find_resolution_choice(&batch).is_some());
    engine.apply_command(0, &choose(1)).unwrap();
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, source),
        ["Chosen opponent: P2"]
    );
    assert!(engine.state.zone_change_generation[&source] > old_generation);
    assert_eq!(engine.state.stack[0].id, old_trigger);
    assert_eq!(engine.state.stack[0].source_zone_change, old_generation);
    assert_eq!(
        engine.state.stack[0].trigger_context.affected_player,
        Some(1)
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 17);
    assert_eq!(engine.state.players[2].life, 20);
    advance_until(&mut engine, 2, TurnStep::Upkeep);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0].source_zone_change,
        engine.state.zone_change_generation[&source]
    );
    assert_eq!(
        engine.state.stack[0].trigger_context.affected_player,
        Some(2)
    );
}

#[test]
fn black_vise_actual_paid_boomerang_and_recast_reset_choice() {
    let (mut engine, source, _) = parked(&[0, 1, 2]);
    engine.apply_command(0, &choose(0)).unwrap();
    inject_card_into_hand(&mut engine, 0, "boomerang");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "boomerang");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, source).is_empty());
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "black_vise");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    engine.apply_command(0, &choose(1)).unwrap();
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, source),
        ["Chosen opponent: P2"]
    );
}

#[test]
fn black_vise_exact_single_face_and_complete_linked_damage_definition() {
    use tricerules_cards::primitives::{
        Amount, ConditionPlayerSet, CountExpression, PlayerRecipient, SpellEffectKind,
        StaticAbilityDef, TriggerCondition,
    };
    let definition = CardRegistry::global().get("black_vise").unwrap();
    assert_eq!(definition.name, "Black Vise");
    assert_eq!(definition.layout, tricerules_cards::Layout::Normal);
    assert_eq!(definition.faces_iter().count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert_eq!((face.power, face.toughness), (None, None));
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(face.triggered_abilities.len(), 1);
    assert!(face.activated_abilities.is_empty() && face.spell_effect.is_empty());
    let StaticAbilityDef::AsEntersChooseOpponent { link_id: producer } =
        &face.static_abilities[0].definition
    else {
        panic!("as-entry linked opponent producer");
    };
    let TriggerCondition::AtBeginningOfChosenPlayerUpkeep { link_id: consumer } =
        &face.triggered_abilities[0].trigger
    else {
        panic!("chosen-player upkeep consumer");
    };
    assert_eq!(producer, consumer);
    assert!(face.triggered_abilities[0].intervening_if.is_none());
    let [SpellEffectKind::DamagePlayer {
        amount: Amount::Count(CountExpression::Affine { constant, terms }),
        who: PlayerRecipient::AffectedPlayer,
    }] = face.triggered_abilities[0].effect.as_slice()
    else {
        panic!("resolution-time affected-player damage");
    };
    assert_eq!(*constant, -4);
    assert_eq!(terms.len(), 1);
    assert_eq!(terms[0].coefficient, 1);
    assert!(matches!(
        terms[0].quantity,
        CountExpression::CardsInHand {
            players: ConditionPlayerSet::AffectedPlayer
        }
    ));
}

#[test]
fn black_vise_replays_exact_logged_paid_cast_choice_departure_and_reentry_commands() {
    use tricerules_proto::ruled::v1::{
        dev_command::Dev, DevAddMana, DevCommand, DevMoveCard, DevPutCardInZone, DevZone,
    };
    fn engine() -> GameEngine {
        let mut engine = GameEngine::new(50604, &[9, 0, 23, 41], 20, None, true).unwrap();
        engine.enable_dev_commands();
        engine
    }
    fn dev(payload: Dev) -> RuledCommand {
        RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: 9,
                dev: Some(payload),
            })),
        }
    }
    let mut first = engine();
    let mut log = Vec::new();
    fn apply(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, RuledCommand, RuledEventBatch, serde_json::Value)>,
        actor: i32,
        command: RuledCommand,
    ) {
        let batch = engine.apply_command(actor, &command).unwrap();
        log.push((actor, command, batch, engine.diagnostic_snapshot().unwrap()));
    }
    while first.state.turn_step != TurnStep::Main1 {
        let actor = first.state.priority_player_id();
        apply(&mut first, &mut log, actor, pass());
    }
    apply(
        &mut first,
        &mut log,
        9,
        dev(Dev::PutCardInZone(DevPutCardInZone {
            card_name: "Black Vise".into(),
            zone: DevZone::Hand as i32,
            ready: false,
        })),
    );
    apply(
        &mut first,
        &mut log,
        9,
        dev(Dev::AddMana(DevAddMana {
            c: 1,
            ..Default::default()
        })),
    );
    let slot = hand_index_for_card(&first, 0, "black_vise");
    apply(&mut first, &mut log, 9, cast_spell(slot, vec![]));
    while first.state.pending_resolution.is_none() {
        let actor = first.state.priority_player_id();
        apply(&mut first, &mut log, actor, pass());
    }
    apply(&mut first, &mut log, 0, concede());
    apply(&mut first, &mut log, 9, choose(1));
    for zone in [DevZone::Hand, DevZone::Battlefield] {
        apply(
            &mut first,
            &mut log,
            9,
            dev(Dev::MoveCard(DevMoveCard {
                card_name: "Black Vise".into(),
                zone: zone as i32,
                ready: false,
            })),
        );
    }
    apply(&mut first, &mut log, 9, choose(1));
    let mut replay = engine();
    for (actor, command, batch, snapshot) in log {
        assert_eq!(replay.apply_command(actor, &command).unwrap(), batch);
        assert_eq!(replay.diagnostic_snapshot().unwrap(), snapshot);
    }
}

#[cfg(feature = "authoring")]
mod copying {
    use super::*;
    use tricerules_core::EngineDeck;

    const COPY: &str = r#"(
        id: "black_vise_copy_fixture", name: "Copy fixture", face_id: "copy_fixture",
        mana_cost: "{0}", types: ["Artifact"],
        static_abilities: [(ability_id: "copy", presentation: Fallback,
            definition: EntersAsCopy(filter: (kind: AnyPermanent)))],
    )"#;
    const TOKENS: &str = r#"(
        id: "black_vise_token_fixture", name: "Token fixture", face_id: "token_fixture",
        mana_cost: "{0}", types: ["Sorcery"],
        spell_effect: [CreateTokenCopies(count: 2, source: Chosen((kind: AnyPermanent)))],
        targeting: (groups: [(min: 1, max: 1, prompt: "Choose a permanent", effect_indices: [0])]),
    )"#;

    fn fixture(draft: &str) -> (GameEngine, u32) {
        let registry = Box::leak(Box::new(
            CardRegistry::from_chunks_and_tokens(
                &[
                    include_str!("../../../tricerules-cards/data/black_vise.ron"),
                    include_str!("../../../tricerules-cards/data/forest.ron"),
                    draft,
                ],
                &[],
            )
            .unwrap(),
        ));
        let decks = vec![
            EngineDeck {
                mainboard: vec!["forest".into(); 40],
                commanders: vec![]
            };
            3
        ];
        let mut engine =
            GameEngine::new_for_authoring(50603, &[0, 1, 2], 20, Some(decks), true, registry)
                .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = inject_card_into_hand(&mut engine, 0, "black_vise");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "black_vise");
        engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
        pass_priority_round(&mut engine);
        engine.apply_command(0, &choose(0)).unwrap();
        (engine, source)
    }

    #[test]
    fn black_vise_copy_first_entrant_chooses_afresh_and_keeps_acquired_occurrence() {
        let (mut engine, source) = fixture(COPY);
        let copy = inject_card_into_hand(&mut engine, 0, "black_vise_copy_fixture");
        let slot = hand_index_for_card(&engine, 0, "black_vise_copy_fixture");
        engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
        pass_priority_round(&mut engine);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            ChoiceKind::CopySource
        );
        let batch = engine
            .apply_command(0, &submit_resolution_choice(vec![source]))
            .unwrap();
        assert_eq!(
            find_resolution_choice(&batch).unwrap().choice_kind,
            ChoiceKind::ResolutionBranch as i32
        );
        assert_eq!(engine.state.objects[&copy].zone, Zone::Stack);
        assert!(!engine.state.players[0].battlefield.contains(&copy));
        engine.apply_command(0, &choose(1)).unwrap();
        assert_eq!(
            zone_view_rules_annotation_labels(&mut engine, 0, source),
            ["Chosen opponent: P1"]
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut engine, 0, copy),
            ["Chosen opponent: P2"]
        );
        assert!(engine.state.objects[&copy].copiable_values.is_some());
        advance_until(&mut engine, 2, TurnStep::Upkeep);
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(engine.state.stack[0].source_permanent_id, Some(copy));
    }

    #[test]
    fn black_vise_two_copied_tokens_choose_independently_before_simultaneous_commit() {
        let (mut engine, source) = fixture(TOKENS);
        inject_card_into_hand(&mut engine, 0, "black_vise_token_fixture");
        let slot = hand_index_for_card(&engine, 0, "black_vise_token_fixture");
        engine
            .apply_command(0, &cast_spell(slot, target_object(source)))
            .unwrap();
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.players[0].battlefield, [source]);
        engine.apply_command(0, &choose(0)).unwrap();
        assert_eq!(
            engine.state.players[0].battlefield,
            [source],
            "all entries remain provisional"
        );
        engine.apply_command(0, &choose(1)).unwrap();
        answer_simultaneous_entry_order_in_engine_order(&mut engine);
        let copies = engine.state.players[0]
            .battlefield
            .iter()
            .copied()
            .filter(|id| *id != source)
            .collect::<Vec<_>>();
        assert_eq!(copies.len(), 2);
        assert_eq!(
            zone_view_rules_annotation_labels(&mut engine, 0, copies[0]),
            ["Chosen opponent: P1"]
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut engine, 0, copies[1]),
            ["Chosen opponent: P2"]
        );
        for copy in copies {
            assert!(engine.state.objects[&copy].token_origin.is_some());
            assert!(engine.state.objects[&copy].copiable_values.is_none());
        }
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn black_vise_four_nonconsecutive_players_choose_zero_and_reject_malformed_answers() {
    let (mut engine, source, batch) = parked(&[9, 0, 23, 41]);
    let options = find_resolution_choice(&batch).unwrap().resolution_branches;
    assert_eq!(
        options
            .iter()
            .map(|option| option.label.as_str())
            .collect::<Vec<_>>(),
        ["P0", "P23", "P41"]
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, source).is_empty());
    reject_unchanged(&mut engine, 0, &choose(0));
    reject_unchanged(&mut engine, 9, &choose(3));
    for answer in [
        SubmitResolutionChoice {
            chosen_object_ids: vec![source],
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            ..Default::default()
        },
        SubmitResolutionChoice {
            chosen_player_ids: vec![0],
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            ..Default::default()
        },
        SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::Unspecified as i32,
            ..Default::default()
        },
        SubmitResolutionChoice {
            payment: Some(Default::default()),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            ..Default::default()
        },
    ] {
        reject_unchanged(
            &mut engine,
            9,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(answer)),
            },
        );
    }
    engine.apply_command(9, &choose(0)).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, source),
        ["Chosen opponent: P0"]
    );
    reject_unchanged(&mut engine, 9, &choose(0));
}

#[test]
fn black_vise_pending_departure_disables_original_branch_without_renumbering() {
    let (mut engine, source, _) = parked(&[9, 0, 23, 41]);
    let batch = engine.apply_command(0, &concede()).unwrap();
    let choice = find_resolution_choice(&batch).expect("refresh after opponent departure");
    assert_eq!(
        choice
            .resolution_branches
            .iter()
            .map(|option| (
                option.branch_index,
                option.label.as_str(),
                option.selectable
            ))
            .collect::<Vec<_>>(),
        [(0, "P0", false), (1, "P23", true), (2, "P41", true)]
    );
    reject_unchanged(&mut engine, 9, &choose(0));
    engine.apply_command(9, &choose(1)).unwrap();
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, source),
        ["Chosen opponent: P23"]
    );
}

#[test]
fn black_vise_paid_cast_parks_before_entry_for_private_opponent_choice() {
    let mut engine = semantic::main_phase(50601);
    let source = inject_card_into_hand(&mut engine, 0, "black_vise");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "black_vise");
    semantic::accepted(&mut engine, 0, &cast_spell(slot, vec![]));
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let mut batch = RuledEventBatch::default();
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        batch = engine.apply_command(actor, &pass()).unwrap();
    }
    let choice = find_resolution_choice(&batch).expect("as-entry opponent choice");
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.source_object_id, source);
    assert_eq!(choice.choice_kind, ChoiceKind::ResolutionBranch as i32);
    assert_eq!(choice.resolution_branches.len(), 1);
    assert_eq!(choice.resolution_branches[0].label, "P1");
    assert!(choice.resolution_branches[0].selectable);
    assert!(choice.candidate_object_ids.is_empty());
    assert!(!engine.state.players[0].battlefield.contains(&source));
}

fn reanimated_copy_pending() -> (GameEngine, u32, u32) {
    let (mut engine, source, _) = parked(&[0, 1, 2, 3]);
    engine.apply_command(0, &choose(0)).unwrap();
    let entrant = inject_graveyard_card(&mut engine, 1, "clever_impersonator");
    let reanimate = inject_card_into_hand(&mut engine, 0, "reanimate");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "reanimate");
    engine
        .apply_command(0, &cast_spell(slot, target_object(entrant)))
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.black, 0);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::CopySource
    );
    let copy_decider = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .deciding_player;
    let batch = engine
        .apply_command(copy_decider, &submit_resolution_choice(vec![source]))
        .unwrap();
    assert_eq!(
        find_resolution_choice(&batch).unwrap().choice_kind,
        ChoiceKind::ResolutionBranch as i32
    );
    assert_eq!(
        find_resolution_choice(&batch).unwrap().deciding_player_id,
        0
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Graveyard);
    (engine, entrant, reanimate)
}

#[test]
fn black_vise_reanimated_copy_owner_departure_finishes_surviving_outer_spell() {
    let (mut engine, entrant, reanimate) = reanimated_copy_pending();
    let batch = engine.apply_command(1, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&entrant));
    assert!(
        engine.state.pending_resolution.is_none(),
        "removed entrant must not strand its surviving outer spell"
    );
    assert!(
        find_resolution_choice(&batch).is_none(),
        "never reoffer an impossible entry choice"
    );
    assert_eq!(engine.state.objects[&reanimate].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0].life, 16,
        "uncommitted entry uses the original graveyard mana value"
    );
    assert_eq!(batch.events.iter().filter(|event| matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == reanimate)).count(), 1);
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .unwrap();
}

#[test]
fn black_vise_successful_reanimated_copy_uses_resulting_mana_value() {
    let (mut engine, entrant, reanimate) = reanimated_copy_pending();
    let batch = engine.apply_command(0, &choose(1)).unwrap();
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&entrant].controller, 0);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, entrant),
        ["Chosen opponent: P2"]
    );
    assert_eq!(
        engine.state.players[0].life, 19,
        "completed copy uses resulting Black Vise mana value"
    );
    assert_eq!(engine.state.objects[&reanimate].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(batch.events.iter().filter(|event| matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == reanimate)).count(), 1);
}

#[test]
fn black_vise_ghost_vacuum_other_entrant_departure_preserves_current_choice() {
    let (mut engine, source, _) = parked(&[0, 1, 2, 3]);
    engine.apply_command(0, &choose(0)).unwrap();
    let vacuum = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let entrant = inject_graveyard_card(&mut engine, 0, "clever_impersonator");
    let departed = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    for target in [entrant, departed] {
        engine.state.objects.get_mut(&vacuum).unwrap().tapped = false;
        let command = activate_ability_for(&engine, vacuum, 0, target_object(target));
        engine.apply_command(0, &command).unwrap();
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    }
    engine.state.objects.get_mut(&vacuum).unwrap().tapped = false;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );
    let command = activate_ability_for(&engine, vacuum, 1, vec![]);
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let outer = engine.state.stack.last().unwrap().id;
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    assert_eq!(
        find_resolution_choice(&batch).unwrap().deciding_player_id,
        0
    );
    let batch = engine.apply_command(1, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&departed));
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Exile);
    let choice = find_resolution_choice(&batch).unwrap();
    assert_eq!(
        choice
            .resolution_branches
            .iter()
            .map(|option| (
                option.branch_index,
                option.label.as_str(),
                option.selectable
            ))
            .collect::<Vec<_>>(),
        [(0, "P1", false), (1, "P2", true), (2, "P3", true)]
    );
    let batch = engine
        .apply_command(0, &choose(1))
        .expect("surviving entry choice must remain answerable after another cohort owner leaves");
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, entrant),
        ["Chosen opponent: P2", "Flying"]
    );
    assert!(!engine.state.objects.contains_key(&departed));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(batch.events.iter().filter(|event| matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == outer)).count(), 1);
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .unwrap();
}
