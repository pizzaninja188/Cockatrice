//! Exact Lantern: paid cast, dynamic granted mana, stable slots and recipient identity.
use super::helpers::*;
use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, AbilityInfo, DevCommand, DevMoveCard, DevZone, ResolutionChoiceDecision,
};

const LANTERN: &str = "chromatic_lantern";

fn setup() -> (GameEngine, u32) {
    assert!(
        CardRegistry::global().get(LANTERN).is_some(),
        "exact Lantern is missing"
    );
    let mut engine = GameEngine::new(
        508_100,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, LANTERN);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, LANTERN);
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    (engine, source)
}

fn catalog(engine: &mut GameEngine, source: u32) -> Vec<AbilityInfo> {
    engine
        .initial_response_batch()
        .events
        .into_iter()
        .find_map(|event| match event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .into_iter()
                .flat_map(|player| player.battlefield_objects)
                .find(|object| object.object_id == source)
                .map(|object| object.activated_abilities),
            _ => None,
        })
        .expect("public recipient catalog")
}

fn granted_slot(engine: &mut GameEngine, source: u32) -> u32 {
    catalog(engine, source)
        .into_iter()
        .find(|info| {
            info.presentation
                .as_ref()
                .is_some_and(|presentation| presentation.card_id == LANTERN)
        })
        .expect("Lantern grants a definition-backed ability")
        .ability_index
}

fn mana(engine: &GameEngine, source: u32, slot: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, slot, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = &mut command.cmd else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

fn move_source(engine: &mut GameEngine, source: u32, zone: DevZone) {
    let card_name = CardRegistry::global()
        .get(&engine.state.objects[&source].card_id)
        .unwrap()
        .name
        .clone();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name,
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn lantern_paid_cast_exact_definition_and_both_oracle_clauses() {
    let (mut engine, source) = setup();
    let card = CardRegistry::global().get(LANTERN).unwrap();
    assert_eq!(card.name, "Chromatic Lantern");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(
        face.supertypes.is_empty() && face.colors().is_empty() && card.color_identity().is_empty()
    );
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(face.activated_abilities[0].costs, [AbilityCost::Tap]);
    assert_eq!(
        face.activated_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let StaticAbilityDef::GrantActivatedAbilityToPermanents {
        filter,
        activated_abilities,
    } = &face.static_abilities[0].definition
    else {
        panic!("scoped grant");
    };
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(filter.permanent_types, [PermanentTypeFilter::Land]);
    assert_eq!(activated_abilities.len(), 1);
    assert_eq!(activated_abilities[0].costs, [AbilityCost::Tap]);
    assert!(!activated_abilities[0].intrinsic_land_mana);
    assert_eq!(catalog(&mut engine, source)[0].ability_index, 0);
}

#[test]
fn lantern_own_and_granted_basic_nonbasic_mana_have_five_immediate_single_outputs_and_undo() {
    for recipient in [LANTERN, "forest", "darksteel_citadel"] {
        for option in 0..5 {
            let (mut engine, lantern) = setup();
            let source = if recipient == LANTERN {
                lantern
            } else {
                inject_permanent_on_battlefield(&mut engine, 0, recipient)
            };
            let slot = if source == lantern {
                0
            } else {
                granted_slot(&mut engine, source)
            };
            engine
                .apply_command(0, &mana(&engine, source, slot, option))
                .unwrap();
            assert!(engine.state.stack.is_empty());
            assert!(engine.state.objects[&source].tapped);
            let pool = &engine.state.players[0].mana_pool;
            assert_eq!(
                [pool.white, pool.blue, pool.black, pool.red, pool.green],
                std::array::from_fn::<_, 5, _>(|index| u32::from(index == option as usize))
            );
            assert_eq!(pool.colorless, 0);
            let already_tapped = mana(&engine, source, slot, option);
            reject(&mut engine, 0, &already_tapped);
            engine.apply_command(0, &undo_mana_ability()).unwrap();
            assert!(!engine.state.objects[&source].tapped);
            let pool = &engine.state.players[0].mana_pool;
            assert_eq!(
                [pool.white, pool.blue, pool.black, pool.red, pool.green],
                [0; 5]
            );
        }
    }
}

#[test]
fn lantern_dynamic_scope_preserves_native_types_and_abilities_and_excludes_opponents_nonlands() {
    let (mut engine, _) = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "darksteel_citadel");
    let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let opposing = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    assert_eq!(catalog(&mut engine, land).len(), 2);
    assert_eq!(catalog(&mut engine, forest).len(), 2);
    assert_eq!(catalog(&mut engine, opposing).len(), 1);
    assert_eq!(catalog(&mut engine, artifact).len(), 1);
    let types = engine.characteristics(land).unwrap();
    assert!(
        types.has_type("Artifact")
            && types.has_type("Land")
            && types.has_keyword(Keyword::Indestructible)
    );
    assert!(!types.has_type("Forest"));
    engine.apply_command(0, &mana(&engine, land, 0, 0)).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
}

#[test]
fn lantern_second_grant_survives_first_departure_without_rebinding_and_new_incarnations_append() {
    let (mut engine, first) = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let first_slot = granted_slot(&mut engine, land);
    let second = inject_card_into_hand(&mut engine, 0, LANTERN);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let hand = hand_index_for_card(&engine, 0, LANTERN);
    engine.apply_command(0, &cast_spell(hand, vec![])).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&second].zone, Zone::Battlefield);
    let slots = catalog(&mut engine, land)
        .into_iter()
        .map(|info| info.ability_index)
        .collect::<Vec<_>>();
    assert_eq!(slots, [0, first_slot, first_slot + 1]);
    let stale = mana(&engine, land, first_slot, 4);
    let survivor = mana(&engine, land, first_slot + 1, 4);
    move_source(&mut engine, first, DevZone::Graveyard);
    assert_eq!(
        catalog(&mut engine, land)
            .iter()
            .map(|info| info.ability_index)
            .collect::<Vec<_>>(),
        [0, first_slot + 1]
    );
    reject(&mut engine, 0, &stale);
    reject(&mut engine, 1, &survivor);
    engine.apply_command(0, &survivor).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    engine.apply_command(0, &undo_mana_ability()).unwrap();
    move_source(&mut engine, first, DevZone::Battlefield);
    assert_eq!(
        catalog(&mut engine, land)
            .iter()
            .map(|info| info.ability_index)
            .collect::<Vec<_>>(),
        [0, first_slot + 1, first_slot + 2]
    );
    let recipient_stale = mana(&engine, land, first_slot + 1, 0);
    move_source(&mut engine, land, DevZone::Graveyard);
    move_source(&mut engine, land, DevZone::Battlefield);
    reject(&mut engine, 0, &recipient_stale);
}

fn temporary_effect(
    engine: &mut GameEngine,
    source: u32,
    kind: ContinuousEffectKind,
    timestamp: u64,
) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp,
    });
}

#[test]
fn lantern_source_suppression_works_before_or_after_grant_and_restores_the_same_slot() {
    for timestamp in [0, 10_000] {
        let (mut engine, lantern) = setup();
        let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        let slot = granted_slot(&mut engine, land);
        temporary_effect(
            &mut engine,
            lantern,
            ContinuousEffectKind::Layer6RemoveAllAbilities,
            timestamp,
        );
        assert_eq!(
            catalog(&mut engine, land)
                .iter()
                .map(|info| info.ability_index)
                .collect::<Vec<_>>(),
            [0]
        );
        let stale = mana(&engine, land, slot, 0);
        reject(&mut engine, 0, &stale);
        engine.state.continuous_effects.retain(|effect| {
            !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
        });
        assert_eq!(granted_slot(&mut engine, land), slot);
        engine
            .apply_command(0, &mana(&engine, land, slot, 0))
            .unwrap();
        assert_eq!(engine.state.players[0].mana_pool.white, 1);
    }
}

#[test]
fn lantern_new_noncreature_land_can_tap_but_sick_creature_land_requires_haste() {
    let (mut engine, _) = setup();
    let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    engine
        .state
        .objects
        .get_mut(&forest)
        .unwrap()
        .summoning_sick = true;
    let forest_slot = granted_slot(&mut engine, forest);
    engine
        .apply_command(0, &mana(&engine, forest, forest_slot, 1))
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    let arbor = inject_permanent_on_battlefield(&mut engine, 0, "dryad_arbor");
    engine.state.objects.get_mut(&arbor).unwrap().summoning_sick = true;
    let slot = granted_slot(&mut engine, arbor);
    assert!(
        !catalog(&mut engine, arbor)
            .iter()
            .find(|info| info.ability_index == slot)
            .unwrap()
            .activatable
    );
    let command = mana(&engine, arbor, slot, 4);
    reject(&mut engine, 0, &command);
    temporary_effect(
        &mut engine,
        arbor,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Haste),
        10_000,
    );
    assert!(
        catalog(&mut engine, arbor)
            .iter()
            .find(|info| info.ability_index == slot)
            .unwrap()
            .activatable
    );
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
}

#[test]
fn lantern_invalid_option_targets_slot_generation_and_actor_are_atomic() {
    let (mut engine, _) = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let slot = granted_slot(&mut engine, land);
    let valid = mana(&engine, land, slot, 0);
    reject(&mut engine, 1, &valid);
    for mut command in [
        mana(&engine, land, slot, 5),
        mana(&engine, land, slot + 100, 0),
    ] {
        reject(&mut engine, 0, &command);
        if let Some(Cmd::ActivateAbility(ability)) = &mut command.cmd {
            ability.expected_zone_change_generation += 1;
        }
        reject(&mut engine, 0, &command);
    }
    let mut target = valid.clone();
    let Some(Cmd::ActivateAbility(ability)) = &mut target.cmd else {
        unreachable!()
    };
    ability.targets = target_object(land);
    reject(&mut engine, 0, &target);
    let mut stale_generation = valid.clone();
    let Some(Cmd::ActivateAbility(ability)) = &mut stale_generation.cmd else {
        unreachable!()
    };
    ability.expected_zone_change_generation += 1;
    reject(&mut engine, 0, &stale_generation);
    engine.apply_command(0, &valid).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.white, 1);
}

#[test]
fn lantern_granted_mana_can_pay_casts_and_parked_resolution_payments_with_undo() {
    use tricerules_proto::ruled::v1::PreviewPayment;
    fn reject_preview(engine: &GameEngine, command: &RuledCommand) {
        let Some(Cmd::ActivateAbility(activation)) = &command.cmd else {
            unreachable!()
        };
        let before = engine.diagnostic_snapshot().unwrap();
        let preview = engine.preview_payment(
            0,
            &PreviewPayment {
                activate_ability: Some(activation.clone()),
                ..Default::default()
            },
        );
        assert!(!preview.valid, "{preview:?}");
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    fn previewed_activation(engine: &GameEngine, command: RuledCommand) -> RuledCommand {
        let Some(Cmd::ActivateAbility(mut activation)) = command.cmd else {
            unreachable!()
        };
        let before = engine.diagnostic_snapshot().unwrap();
        let proposal = PreviewPayment {
            transaction_id: 508,
            revision: engine.state.command_index,
            activate_ability: Some(activation.clone()),
            ..Default::default()
        };
        let wrong_actor = engine.preview_payment(1, &proposal);
        assert!(!wrong_actor.valid, "{wrong_actor:?}");
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        let preview = engine.preview_payment(0, &proposal);
        assert!(preview.valid && preview.complete, "{preview:?}");
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        activation.payment = preview.selection;
        activation.restricted_mana = preview.restricted_mana;
        RuledCommand {
            cmd: Some(Cmd::ActivateAbility(activation)),
        }
    }
    let (mut engine, _) = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let slot = granted_slot(&mut engine, land);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "unsummon");
    let hand = hand_index_for_card(&engine, 0, "unsummon");
    let cast = cast_spell(hand, target_object(target));
    reject(&mut engine, 0, &cast);
    engine
        .apply_command(0, &mana(&engine, land, slot, 1))
        .unwrap();
    engine.apply_command(0, &undo_mana_ability()).unwrap();
    reject(&mut engine, 0, &cast);
    engine
        .apply_command(0, &mana(&engine, land, slot, 1))
        .unwrap();
    engine.apply_command(0, &cast).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);

    for (pay, active_player) in [(false, 0), (true, 0), (false, 1), (true, 1)] {
        let (mut engine, _) = setup();
        engine.state.active_player_idx = active_player;
        let first = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        let second = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        let first_slot = granted_slot(&mut engine, first);
        let second_slot = granted_slot(&mut engine, second);
        let shredder = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
        let nonmana = activate_ability_for(&engine, shredder, 0, target_player(1));
        // This otherwise legal activation becomes illegal while the mana payment is parked.
        let _ = previewed_activation(&engine, nonmana.clone());
        let warded =
            inject_permanent_on_battlefield(&mut engine, 1, "dirgur_island_dragon_skimming_strike");
        inject_card_into_hand(&mut engine, 0, "unsummon");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let hand = hand_index_for_card(&engine, 0, "unsummon");
        engine
            .apply_command(0, &cast_spell(hand, target_object(warded)))
            .unwrap();
        pass_both_players(&mut engine);
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(engine.state.priority_player_id(), active_player as i32);
        reject_preview(&engine, &nonmana);
        reject(&mut engine, 0, &nonmana);
        let activation = previewed_activation(&engine, mana(&engine, first, first_slot, 4));
        engine.apply_command(0, &activation).unwrap();
        engine.apply_command(0, &undo_mana_ability()).unwrap();
        assert!(!engine.state.objects[&first].tapped);
        let activation = previewed_activation(&engine, mana(&engine, first, first_slot, 4));
        engine.apply_command(0, &activation).unwrap();
        let activation = previewed_activation(&engine, mana(&engine, second, second_slot, 1));
        engine.apply_command(0, &activation).unwrap();
        submit_mana_resolution_decision(
            &mut engine,
            0,
            if pay {
                ResolutionChoiceDecision::PayMana
            } else {
                ResolutionChoiceDecision::Decline
            },
        )
        .unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&first].tapped, pay);
        assert_eq!(engine.state.objects[&second].tapped, pay);
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&warded].zone,
            if pay { Zone::Hand } else { Zone::Battlefield }
        );
    }

    let (mut engine, _) = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let slot = granted_slot(&mut engine, land);
    inject_card_into_hand(&mut engine, 0, "brainstorm");
    cast_instant_and_resolve(
        &mut engine,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert!(pending.continuation.mana_window_undo_start().is_none());
    let activation = mana(&engine, land, slot, 1);
    reject_preview(&engine, &activation);
    reject(&mut engine, 0, &activation);
}

#[test]
fn lantern_replays_paid_cast_grants_undo_and_new_source_incarnation() {
    use tricerules_proto::ruled::v1::{dev_command::Dev, DevAddMana, DevPutCardInZone};
    fn game() -> GameEngine {
        let mut engine = GameEngine::new(
            508_108,
            &[0, 1],
            20,
            Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
            true,
        )
        .unwrap();
        engine.enable_dev_commands();
        engine
    }
    fn dev(payload: Dev) -> RuledCommand {
        RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: 0,
                dev: Some(payload),
            })),
        }
    }
    fn apply(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, RuledCommand, RuledEventBatch, serde_json::Value)>,
        actor: i32,
        command: RuledCommand,
    ) {
        let batch = engine.apply_command(actor, &command).unwrap();
        log.push((actor, command, batch, engine.diagnostic_snapshot().unwrap()));
    }
    let mut engine = game();
    let mut log = Vec::new();
    while engine.state.turn_step != tricerules_core::TurnStep::Main1 {
        let actor = engine.state.priority_player_id();
        apply(&mut engine, &mut log, actor, pass());
    }
    apply(
        &mut engine,
        &mut log,
        0,
        dev(Dev::PutCardInZone(DevPutCardInZone {
            card_name: "Chromatic Lantern".into(),
            zone: DevZone::Hand as i32,
            ready: false,
        })),
    );
    apply(
        &mut engine,
        &mut log,
        0,
        dev(Dev::AddMana(DevAddMana {
            c: 3,
            ..Default::default()
        })),
    );
    let hand = hand_index_for_card(&engine, 0, LANTERN);
    apply(&mut engine, &mut log, 0, cast_spell(hand, vec![]));
    for _ in 0..8 {
        if engine.state.stack.is_empty() {
            break;
        }
        let actor = engine.state.priority_player_id();
        apply(&mut engine, &mut log, actor, pass());
    }
    assert!(engine.state.stack.is_empty());
    apply(
        &mut engine,
        &mut log,
        0,
        dev(Dev::PutCardInZone(DevPutCardInZone {
            card_name: "Forest".into(),
            zone: DevZone::Battlefield as i32,
            ready: true,
        })),
    );
    let land = battlefield_object_for_card(&engine, 0, "forest");
    let command = mana(&engine, land, 1, 1);
    apply(&mut engine, &mut log, 0, command);
    apply(&mut engine, &mut log, 0, undo_mana_ability());
    apply(
        &mut engine,
        &mut log,
        0,
        dev(Dev::MoveCard(DevMoveCard {
            card_name: "Chromatic Lantern".into(),
            zone: DevZone::Graveyard as i32,
            ready: false,
        })),
    );
    let stale = mana(&engine, land, 1, 0);
    reject(&mut engine, 0, &stale);
    apply(
        &mut engine,
        &mut log,
        0,
        dev(Dev::MoveCard(DevMoveCard {
            card_name: "Chromatic Lantern".into(),
            zone: DevZone::Battlefield as i32,
            ready: true,
        })),
    );
    let command = mana(&engine, land, 2, 3);
    apply(&mut engine, &mut log, 0, command);
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    let mut replay = game();
    for (actor, command, batch, snapshot) in log {
        assert_eq!(replay.apply_command(actor, &command).unwrap(), batch);
        assert_eq!(replay.diagnostic_snapshot().unwrap(), snapshot);
    }
}
#[test]
fn lantern_granted_mana_pays_an_open_spell_transaction_with_preview_and_undo() {
    use tricerules_proto::ruled::v1::{
        BeginSpellCast, CommitSpellCast, PaymentMana, PaymentSelection, PreviewPayment,
        SpellCastAnnouncement,
    };
    let (mut engine, _) = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let slot = granted_slot(&mut engine, land);
    let shredder = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    let nonmana = activate_ability_for(&engine, shredder, 0, target_player(1));
    let target = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let spell = inject_card_into_hand(&mut engine, 0, "unsummon");
    let hand = hand_index_for_card(&engine, 0, "unsummon");
    let Some(Cmd::CastSpell(cast)) = cast_spell(hand, target_object(target)).cmd else {
        unreachable!()
    };
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                    announcement: Some(SpellCastAnnouncement {
                        targets: cast.targets,
                        x_value: cast.x_value,
                        flex_payments: cast.flex_payments,
                        face_index: cast.face_index,
                        selected_modes: cast.selected_modes,
                        source: cast.source,
                        cost_selections: cast.cost_selections,
                        cast_cost_group_selections: cast.cast_cost_group_selections,
                        cast_method: cast.cast_method,
                        casting_permission_id: cast.casting_permission_id,
                    }),
                })),
            },
        )
        .unwrap();
    let transaction = engine.state.pending_spell_cast.as_ref().unwrap();
    let transaction_id = transaction.transaction_id;
    assert_eq!(transaction.reserved_object_id, spell);
    // Announcement reserves the spell in the stack zone; the committed stack item waits for payment.
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert!(engine.state.stack.is_empty());
    let Some(Cmd::ActivateAbility(nonmana_activation)) = &nonmana.cmd else {
        unreachable!()
    };
    let before = engine.diagnostic_snapshot().unwrap();
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            activate_ability: Some(nonmana_activation.clone()),
            ..Default::default()
        },
    );
    assert!(!preview.valid, "{preview:?}");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    reject(&mut engine, 0, &nonmana);
    let proposed = CommitSpellCast {
        transaction_id,
        payment: Some(PaymentSelection {
            mana: Some(PaymentMana {
                u: 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let commit = RuledCommand {
        cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
            transaction_id,
            ..Default::default()
        })),
    };
    reject(&mut engine, 0, &commit);
    let mut activation = mana(&engine, land, slot, 1);
    reject(&mut engine, 1, &activation);
    let Some(Cmd::ActivateAbility(ability)) = &mut activation.cmd else {
        unreachable!()
    };
    let before = engine.diagnostic_snapshot().unwrap();
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            activate_ability: Some(ability.clone()),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    ability.payment = preview.selection;
    ability.restricted_mana = preview.restricted_mana;
    engine.apply_command(0, &activation).unwrap();
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id,
        transaction_id
    );
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    assert!(engine.state.objects[&land].tapped);
    assert!(engine.state.stack.is_empty());
    let before = engine.diagnostic_snapshot().unwrap();
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id,
            revision: engine.state.command_index,
            commit_spell_cast: Some(proposed.clone()),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine.apply_command(0, &undo_mana_ability()).unwrap();
    assert!(!engine.state.objects[&land].tapped);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id,
        transaction_id
    );
    reject(&mut engine, 0, &commit);
    assert_eq!(granted_slot(&mut engine, land), slot);
    engine
        .apply_command(0, &mana(&engine, land, slot, 1))
        .unwrap();
    // Undo and the retry changed the revision; use a fresh engine-normalized selection.
    let before = engine.diagnostic_snapshot().unwrap();
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id,
            revision: engine.state.command_index,
            commit_spell_cast: Some(proposed),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .unwrap();
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert!(engine.state.objects[&land].tapped);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}
#[test]
fn lantern_recipient_scope_loss_rejects_the_old_slot_and_restoration_reuses_it() {
    for kind in [
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
    ] {
        let (mut engine, _) = setup();
        let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        let slot = granted_slot(&mut engine, land);
        let stale = mana(&engine, land, slot, 1);
        temporary_effect(&mut engine, land, kind.clone(), 10_000);
        assert!(!catalog(&mut engine, land)
            .iter()
            .any(|info| info.ability_index == slot));
        reject(&mut engine, 0, &stale);
        reject(&mut engine, 1, &stale);
        engine
            .state
            .continuous_effects
            .retain(|effect| effect.kind != kind);
        assert_eq!(granted_slot(&mut engine, land), slot);
        engine.apply_command(0, &stale).unwrap();
        assert_eq!(engine.state.players[0].mana_pool.blue, 1);
        assert!(engine.state.objects[&land].tapped);
    }
}

#[test]
fn lantern_recipient_removal_respects_timestamp_and_equal_timestamp_insertion() {
    for (offset, removal_first, expected_grant) in [
        (-1, false, true),
        (1, false, false),
        (0, true, true),
        (0, false, false),
    ] {
        let (mut engine, lantern) = setup();
        let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        let slot = granted_slot(&mut engine, land);
        let grant_position = engine
            .state
            .continuous_effects
            .iter()
            .position(|effect| {
                effect.source_id == Some(lantern)
                    && matches!(effect.kind, ContinuousEffectKind::GrantActivatedAbility(_))
            })
            .unwrap();
        let grant_timestamp = engine.state.continuous_effects[grant_position].timestamp;
        let timestamp = match offset {
            -1 => grant_timestamp.checked_sub(1).unwrap(),
            1 => grant_timestamp + 1,
            _ => grant_timestamp,
        };
        temporary_effect(
            &mut engine,
            land,
            ContinuousEffectKind::Layer6RemoveAllAbilities,
            timestamp,
        );
        if removal_first {
            let removal = engine.state.continuous_effects.pop().unwrap();
            engine
                .state
                .continuous_effects
                .insert(grant_position, removal);
        }
        let entries = catalog(&mut engine, land);
        assert_eq!(
            entries
                .iter()
                .map(|info| info.ability_index)
                .collect::<Vec<_>>(),
            if expected_grant { vec![slot] } else { vec![] }
        );
        let command = mana(&engine, land, slot, 0);
        if expected_grant {
            engine.apply_command(0, &command).unwrap();
            assert_eq!(engine.state.players[0].mana_pool.white, 1);
            engine.apply_command(0, &undo_mana_ability()).unwrap();
        } else {
            reject(&mut engine, 0, &command);
        }
        engine.state.continuous_effects.retain(|effect| {
            !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
        });
        assert_eq!(granted_slot(&mut engine, land), slot);
        assert_eq!(
            catalog(&mut engine, land)
                .iter()
                .map(|info| info.ability_index)
                .collect::<Vec<_>>(),
            [0, slot]
        );
    }
}

#[test]
fn lantern_settled_reconnect_publication_is_idempotent_without_new_slots_or_state() {
    let (mut engine, _) = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let slot = granted_slot(&mut engine, land);
    let before = engine.diagnostic_snapshot().unwrap();
    let first = engine.initial_response_batch();
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let second = engine.initial_response_batch();
    assert_eq!(second, first);
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    assert_eq!(granted_slot(&mut engine, land), slot);
}
