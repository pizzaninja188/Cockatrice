//! Arcane Signet and Command Tower choose mana from the engine-bound Commander identity.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Color;
use tricerules_core::EngineDeck;

fn commander_mana_engine() -> GameEngine {
    commander_mana_engine_with_commanders(&["atraxa,_praetors_voice"])
}

fn commander_mana_engine_with_commanders(commanders: &[&str]) -> GameEngine {
    let mut engine = commander_mana_engine_at_game_start_with_commanders(commanders);
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn commander_mana_engine_at_game_start_with_commanders(commanders: &[&str]) -> GameEngine {
    let mainboard = [
        "arcane_signet",
        "command_tower",
        "forest",
        "forest",
        "forest",
        "forest",
        "forest",
        "forest",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let decks = Some(vec![
        EngineDeck {
            mainboard,
            commanders: commanders.iter().map(|name| (*name).to_owned()).collect(),
        },
        EngineDeck {
            mainboard: vec!["island".to_owned(); 8],
            commanders: vec!["kami_of_the_crescent_moon".to_owned()],
        },
    ]);
    GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        62026,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("both Commander declarations resolve from card data")
}

fn activate_for_mana(
    engine: &mut GameEngine,
    source: u32,
    option_index: u32,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option_index;
    semantic::accepted(engine, 0, &command)
}

fn is_scry_one_trigger(item: &tricerules_core::state::StackItem) -> bool {
    item.triggered_ability.as_ref().is_some_and(|ability| {
        ability.effect
            == [tricerules_cards::primitives::SpellEffectKind::Scry {
                count: tricerules_cards::primitives::Amount::Fixed(1),
            }]
    })
}

fn path_scry_triggers(engine: &GameEngine) -> Vec<&tricerules_core::state::StackItem> {
    engine
        .state
        .stack
        .iter()
        .filter(|item| is_scry_one_trigger(item))
        .collect()
}

fn cast_with_path_mana(
    engine: &mut GameEngine,
    card_id: &str,
    group: u32,
    mut amount: ManaSpendSelection,
) -> u32 {
    let object = inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    let mut command = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    amount.restriction_group_id = group;
    cast.restricted_mana.push(amount);
    semantic::accepted(engine, 0, &command);
    object
}

fn begin_spell_cast_for_path_type_test(
    engine: &mut GameEngine,
    card_id: &str,
    face_index: u32,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) -> (u64, u32) {
    use tricerules_proto::ruled::v1::{BeginSpellCast, SpellCastAnnouncement};

    let object = inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    let Some(Cmd::CastSpell(mut cast)) = cast_spell(slot, vec![]).cmd else {
        unreachable!()
    };
    cast.face_index = face_index;
    cast.targets = targets;
    engine
        .apply_scenario_command(
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
                        prototyped: cast.prototyped,
                        cost_selections: cast.cost_selections,
                        cast_cost_group_selections: cast.cast_cost_group_selections,
                        cast_method: cast.cast_method,
                        casting_permission_id: cast.casting_permission_id,
                    }),
                })),
            },
        )
        .expect("creature spell announcement opens a payment transaction");
    let pending = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("spell payment transaction");
    assert_eq!(pending.reserved_object_id, object);
    (pending.transaction_id, object)
}

fn commit_path_spell_with_mana(
    engine: &mut GameEngine,
    transaction_id: u64,
    group: u32,
    restricted: ManaSpendSelection,
    ordinary: tricerules_proto::ruled::v1::PaymentMana,
) {
    use tricerules_proto::ruled::v1::{CommitSpellCast, CostObjectRef, PaymentSelection};

    let mut restricted = restricted;
    restricted.restriction_group_id = group;
    let source_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("spell payment transaction")
        .reserved_object_id;
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: Some(PaymentSelection {
                        expected_state_revision: engine.state.command_index,
                        source: Some(CostObjectRef {
                            object_id: source_id,
                            zone_change_generation: engine
                                .state
                                .zone_change_generation
                                .get(&source_id)
                                .copied()
                                .unwrap_or(0),
                        }),
                        mana: Some(ordinary),
                        ..Default::default()
                    }),
                    restricted_mana: vec![restricted],
                })),
            },
        )
        .expect("Path mana and ordinary mana commit the spell payment");
}

fn resolve_scry_triggers_and_stack(engine: &mut GameEngine) {
    semantic::complete(engine, 48, |engine| {
        let pending = engine.state.pending_resolution.as_ref()?;
        let chosen = match &pending.continuation {
            ResolutionContinuation::LibraryPartition {
                stage: PendingLibraryPartitionStage::ChooseDestination,
                kind: PendingLibraryPartitionKind::Scry,
                ..
            } => vec![],
            ResolutionContinuation::LibraryPartition {
                stage: PendingLibraryPartitionStage::OrderTop,
                kind: PendingLibraryPartitionKind::Scry,
                ..
            } => pending.presentation.candidates.clone(),
            _ => return None,
        };
        Some((pending.deciding_player, submit_resolution_choice(chosen)))
    })
    .require_exercised();
}

fn move_battlefield_card_to_graveyard_fixture(
    engine: &mut GameEngine,
    player: usize,
    object_id: u32,
) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].graveyard.push(object_id);
    let object = engine.state.objects.get_mut(&object_id).unwrap();
    object.zone = tricerules_core::Zone::Graveyard;
    object.tapped = false;
    let generation = engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_insert(0);
    *generation += 1;
    engine.initial_response_batch();
}

#[test]
fn signet_and_tower_publish_and_apply_commander_identity_choices() {
    let mut engine = commander_mana_engine();
    let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    let tower = inject_permanent_on_battlefield(&mut engine, 0, "command_tower");

    let batch = engine.initial_response_batch();
    let zone_view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("engine publishes a ruled zone view");
    for source in [signet, tower] {
        let info = zone_view.per_player[0]
            .battlefield_objects
            .iter()
            .find(|object| object.object_id == source)
            .expect("mana source appears in the controller's zone view")
            .activated_abilities
            .first()
            .expect("mana ability appears in the zone view");
        assert!(info.is_mana_ability);
        assert!(info.activatable);
        assert_eq!(info.mana_produced, "W/U/B/G");
    }

    // Atraxa is WUBG: choose B from Signet's third published option and G from Tower's fourth.
    activate_for_mana(&mut engine, signet, 2);
    activate_for_mana(&mut engine, tower, 3);
    assert!(engine.state.objects[&signet].tapped);
    assert!(engine.state.objects[&tower].tapped);
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
}

#[test]
fn same_player_two_commander_declaration_unions_color_identity_for_mana_choices() {
    // Commander legality such as Partner is not validated by this engine setup path; this
    // acceptance test covers only the engine's declared-card identity union.
    let mut engine =
        commander_mana_engine_with_commanders(&["atraxa,_praetors_voice", "daretti,_scrap_savant"]);
    assert_eq!(
        engine.state.players[0].color_identity,
        [
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green
        ]
    );
    let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    let tower = inject_permanent_on_battlefield(&mut engine, 0, "command_tower");
    let batch = engine.initial_response_batch();
    let zone_view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("engine publishes a ruled zone view");
    let signet_info = zone_view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == signet)
        .unwrap()
        .activated_abilities
        .first()
        .unwrap();
    assert_eq!(signet_info.mana_produced, "W/U/B/R/G");
    activate_for_mana(&mut engine, signet, 3);
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    let tower_info = zone_view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == tower)
        .unwrap()
        .activated_abilities
        .first()
        .unwrap();
    assert_eq!(tower_info.mana_produced, "W/U/B/R/G");
    activate_for_mana(&mut engine, tower, 3);
    assert!(engine.state.objects[&signet].tapped);
    assert!(engine.state.objects[&tower].tapped);
    assert_eq!(engine.state.players[0].mana_pool.red, 2);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn empty_commander_identity_keeps_signet_and_tower_legal_without_adding_mana() {
    // Eldrazi Devastator is a registry-backed colorless identity fixture. Engine deck setup binds
    // identity but does not enforce Commander eligibility, so this does not assert it is a legal
    // Commander card.
    for commanders in [vec![], vec!["eldrazi_devastator"]] {
        let mut engine = commander_mana_engine_with_commanders(&commanders);
        assert!(engine.state.players[0].color_identity.is_empty());
        let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
        let tower = inject_permanent_on_battlefield(&mut engine, 0, "command_tower");
        let batch = engine.initial_response_batch();
        let zone_view = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ZoneView(view)) => Some(view),
                _ => None,
            })
            .expect("engine publishes a ruled zone view");

        for source in [signet, tower] {
            let info = zone_view.per_player[0]
                .battlefield_objects
                .iter()
                .find(|object| object.object_id == source)
                .unwrap()
                .activated_abilities
                .first()
                .unwrap();
            assert!(info.is_mana_ability);
            assert!(info.activatable);
            assert!(info.mana_produced.is_empty());
        }

        let stack_len = engine.state.stack.len();
        activate_for_mana(&mut engine, signet, 0);
        activate_for_mana(&mut engine, tower, 0);
        assert!(engine.state.objects[&signet].tapped);
        assert!(engine.state.objects[&tower].tapped);
        assert_eq!(engine.state.players[0].mana_pool, Default::default());
        assert_eq!(engine.state.stack.len(), stack_len);
    }
}

#[test]
fn path_of_ancestry_triggers_once_for_each_mana_spent_to_cast_the_commander() {
    let mut engine = commander_mana_engine();
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");

    // This fixture represents the declared commander having been returned to its owner's hand;
    // keep the same physical object and commander designation across the zone change.
    let commander = engine.state.players[0].command_zone.remove(0);
    engine.state.players[0].hand.push(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = tricerules_core::Zone::Hand;
    let generation = engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_insert(0);
    *generation += 1;
    engine.initial_response_batch();

    activate_for_mana(&mut engine, path, 3); // Atraxa's green identity option.
    let restricted = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path of Ancestry mana retains its per-activation rule");
    let group = restricted.restriction_group_id;
    assert_eq!(restricted.amount.g, 1);

    // The Path mana pays the green pip. The ordinary pool pays the remaining four pips.
    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            w: 1,
            u: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|object| *object == commander)
        .expect("returned commander is in hand");
    let command_index = engine.state.command_index;
    let mut forged = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(command)) = forged.cmd.as_mut() else {
        unreachable!()
    };
    command.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group + 100,
        g: 1,
        ..Default::default()
    });
    assert!(engine.apply_scenario_command(0, &forged).is_err());
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.players[0].restricted_mana.len(), 1);
    assert!(path_scry_triggers(&engine).is_empty());

    let mut cast = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(command)) = cast.cmd.as_mut() else {
        unreachable!()
    };
    command.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group,
        g: 1,
        ..Default::default()
    });
    semantic::accepted(&mut engine, 0, &cast);

    let triggers = path_scry_triggers(&engine);
    assert_eq!(triggers.len(), 1);
    let trigger = triggers[0];
    assert_eq!(trigger.source_permanent_id, Some(path));
    assert_eq!(trigger.controller, 0);
    assert_eq!(
        engine.state.objects[&commander].zone,
        tricerules_core::Zone::Stack
    );
    resolve_scry_triggers_and_stack(&mut engine);
    assert_eq!(
        engine.state.objects[&commander].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn path_of_ancestry_triggers_for_a_changeling_creature_spell() {
    let mut engine = commander_mana_engine();
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    activate_for_mana(&mut engine, path, 3);
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path mana group")
        .restriction_group_id;
    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    cast_with_path_mana(
        &mut engine,
        "changeling_wayfinder",
        group,
        ManaSpendSelection {
            g: 1,
            ..Default::default()
        },
    );
    assert_eq!(path_scry_triggers(&engine).len(), 1);
}

#[test]
fn path_of_ancestry_enters_tapped() {
    let mut engine = commander_mana_engine();
    let path = inject_card_into_hand(&mut engine, 0, "path_of_ancestry");
    let slot = hand_index_for_card(&engine, 0, "path_of_ancestry");
    semantic::accepted(&mut engine, 0, &play_land(slot));

    assert_eq!(
        engine.state.objects[&path].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&path].tapped);
}

#[test]
fn path_of_ancestry_does_not_scry_for_nonmatching_or_noncreature_spells() {
    for (card_id, ordinary_mana) in [
        (
            "grizzly_bears",
            super::helpers::ManaGift {
                c: 1,
                ..Default::default()
            },
        ),
        ("sol_ring", super::helpers::ManaGift::default()),
    ] {
        let mut engine = commander_mana_engine();
        let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
        activate_for_mana(&mut engine, path, 3);
        let group = engine.state.players[0]
            .restricted_mana
            .last()
            .expect("Path mana group")
            .restriction_group_id;
        super::helpers::give_mana(&mut engine, 0, ordinary_mana);
        cast_with_path_mana(
            &mut engine,
            card_id,
            group,
            ManaSpendSelection {
                g: 1,
                ..Default::default()
            },
        );
        assert!(path_scry_triggers(&engine).is_empty(), "{card_id}");
    }
}

#[test]
fn path_of_ancestry_checks_current_commander_types_at_cast_time() {
    let mut engine = commander_mana_engine();
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");

    let commander = engine.state.players[0].command_zone.remove(0);
    engine.state.players[0].battlefield.push(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = tricerules_core::Zone::Battlefield;
    let generation = engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_insert(0);
    *generation += 1;
    engine.initial_response_batch();

    activate_for_mana(&mut engine, path, 3);
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path mana group")
        .restriction_group_id;

    // Atraxa has no Bear type when Path produces mana. A later layer-4 type change makes the
    // creature spell share a type at the cast event, so querying only at activation would miss it.
    engine
        .state
        .continuous_effects
        .push(tricerules_core::state::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::state::AffectedScope::Single(commander),
            kind: tricerules_cards::primitives::ContinuousEffectKind::Layer4SetCreatureTypes(vec![
                "Bear".into(),
            ]),
            condition: None,
            duration: tricerules_cards::primitives::EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index + 1,
        });
    assert!(engine.characteristics(commander).unwrap().has_type("Bear"));

    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    cast_with_path_mana(
        &mut engine,
        "grizzly_bears",
        group,
        ManaSpendSelection {
            g: 1,
            ..Default::default()
        },
    );
    assert_eq!(path_scry_triggers(&engine).len(), 1);
}

#[test]
fn path_of_ancestry_uses_current_stack_spell_types_for_its_scry_trigger() {
    use tricerules_proto::ruled::v1::PaymentMana;

    for (card_id, current_type, should_scry, ordinary) in [
        (
            "grizzly_bears",
            "Horror",
            true,
            PaymentMana {
                c: 1,
                ..Default::default()
            },
        ),
        (
            "chasm_skulker",
            "Bear",
            false,
            PaymentMana {
                c: 1,
                u: 1,
                ..Default::default()
            },
        ),
    ] {
        let mut engine = commander_mana_engine();
        let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
        activate_for_mana(&mut engine, path, 3);
        let group = engine.state.players[0]
            .restricted_mana
            .last()
            .expect("Path mana for this spell")
            .restriction_group_id;
        let (transaction_id, spell) =
            begin_spell_cast_for_path_type_test(&mut engine, card_id, 0, vec![]);
        engine
            .state
            .continuous_effects
            .push(tricerules_core::state::ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: tricerules_core::state::AffectedScope::Single(spell),
                kind: tricerules_cards::primitives::ContinuousEffectKind::Layer4SetCreatureTypes(
                    vec![current_type.into()],
                ),
                condition: None,
                duration: tricerules_cards::primitives::EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index + 1,
            });
        assert!(engine
            .characteristics(spell)
            .unwrap()
            .has_type(current_type));

        super::helpers::give_mana(
            &mut engine,
            0,
            super::helpers::ManaGift {
                c: ordinary.c,
                u: ordinary.u,
                ..Default::default()
            },
        );
        commit_path_spell_with_mana(
            &mut engine,
            transaction_id,
            group,
            ManaSpendSelection {
                g: 1,
                ..Default::default()
            },
            ordinary,
        );
        assert_eq!(
            path_scry_triggers(&engine).len(),
            should_scry as usize,
            "{card_id}"
        );
        if should_scry {
            resolve_scry_triggers_and_stack(&mut engine);
        }
    }
}

#[test]
fn path_of_ancestry_ignores_an_unselected_adventure_creature_face() {
    use tricerules_proto::ruled::v1::PaymentMana;

    let mut engine = commander_mana_engine();
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");

    let commander = engine.state.players[0].command_zone.remove(0);
    engine.state.players[0].battlefield.push(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = tricerules_core::Zone::Battlefield;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_insert(0) += 1;
    engine.initial_response_batch();
    engine
        .state
        .continuous_effects
        .push(tricerules_core::state::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::state::AffectedScope::Single(commander),
            kind: tricerules_cards::primitives::ContinuousEffectKind::Layer4SetCreatureTypes(vec![
                "Giant".into(),
            ]),
            condition: None,
            duration: tricerules_cards::primitives::EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index + 1,
        });
    assert!(engine.characteristics(commander).unwrap().has_type("Giant"));

    activate_for_mana(&mut engine, path, 3);
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path produces a restricted mana unit")
        .restriction_group_id;
    let (transaction_id, spell) = begin_spell_cast_for_path_type_test(
        &mut engine,
        "bonecrusher_giant_stomp",
        1,
        target_player(1),
    );
    // The reserved physical card still exposes its front face through object characteristics.
    // The selected Adventure face is Stomp, an instant on the stack.
    assert!(engine.characteristics(spell).unwrap().has_type("Creature"));

    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    commit_path_spell_with_mana(
        &mut engine,
        transaction_id,
        group,
        ManaSpendSelection {
            g: 1,
            ..Default::default()
        },
        PaymentMana {
            r: 1,
            ..Default::default()
        },
    );
    assert!(path_scry_triggers(&engine).is_empty());
}

#[test]
fn path_of_ancestry_checks_the_selected_adventure_face_of_a_commander_on_the_stack() {
    use tricerules_proto::ruled::v1::PaymentMana;

    let mut engine =
        commander_mana_engine_with_commanders(&["smaug,_the_great_calamity_spew_flame"]);
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");

    // Smaug is a supported legendary Dragon commander. This fixture represents it returned to
    // its owner's hand before casting Spew Flame as its Adventure face.
    let commander = engine.state.players[0].command_zone.remove(0);
    engine.state.players[0].hand.push(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = tricerules_core::Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_insert(0) += 1;
    let target = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.initial_response_batch();
    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );
    let adventure_slot = hand_index_for_card(&engine, 0, "smaug,_the_great_calamity_spew_flame");
    semantic::accepted(
        &mut engine,
        0,
        &cast_spell_face(adventure_slot, target_object(target), 1),
    );
    let pending_adventure = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == commander)
        .expect("the declared commander remains on the stack as Spew Flame");
    assert_eq!(pending_adventure.face_index, 1);

    activate_for_mana(&mut engine, path, 0); // Smaug's red color identity option.
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path mana group")
        .restriction_group_id;
    let (transaction_id, spell) =
        begin_spell_cast_for_path_type_test(&mut engine, "hired_blade", 0, vec![]);
    engine
        .state
        .continuous_effects
        .push(tricerules_core::state::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::state::AffectedScope::Single(spell),
            kind: tricerules_cards::primitives::ContinuousEffectKind::Layer4SetCreatureTypes(vec![
                "Dragon".into(),
            ]),
            condition: None,
            duration: tricerules_cards::primitives::EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index + 1,
        });
    assert!(engine.characteristics(spell).unwrap().has_type("Dragon"));
    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    commit_path_spell_with_mana(
        &mut engine,
        transaction_id,
        group,
        ManaSpendSelection {
            r: 1,
            ..Default::default()
        },
        PaymentMana {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    // Spew Flame is a Sorcery Adventure on the stack, so Smaug currently has no creature types.
    assert!(path_scry_triggers(&engine).is_empty());
}

#[test]
fn identical_path_groups_publish_distinct_source_labels() {
    let mut engine = commander_mana_engine();
    let first = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    activate_for_mana(&mut engine, first, 3);
    let response = activate_for_mana(&mut engine, second, 3);
    let pool = response
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ManaPoolUpdated(pool)) if pool.player_id == 0 => Some(pool),
            _ => None,
        })
        .expect("initial response publishes the player's restricted mana");
    let groups = &pool.restricted_groups;
    assert_eq!(groups.len(), 2);
    assert_ne!(groups[0].display_label, groups[1].display_label);
    let first_presentation = groups[0]
        .presentation
        .as_ref()
        .expect("source-bound presentation");
    let second_presentation = groups[1]
        .presentation
        .as_ref()
        .expect("source-bound presentation");
    assert!(first_presentation.oracle_line_indices.is_empty());
    assert!(second_presentation.oracle_line_indices.is_empty());
    assert_ne!(
        first_presentation.fallback_text,
        second_presentation.fallback_text
    );
    assert!(first_presentation
        .fallback_text
        .contains(&first.to_string()));
    assert!(second_presentation
        .fallback_text
        .contains(&second.to_string()));
}

#[test]
fn attack_payment_undo_keeps_the_later_path_mana_group_bound_to_its_source() {
    use tricerules_proto::ruled::v1::{
        CommitAttackDeclaration, PaymentMana, PaymentSelection, UndoManaAbility,
    };

    let mut engine =
        commander_mana_engine_at_game_start_with_commanders(&["atraxa,_praetors_voice"]);
    advance_to_declare_attackers(&mut engine);
    let attacker = *engine.state.players[0]
        .battlefield
        .last()
        .expect("fixture attacker");
    let first = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    inject_permanent_on_battlefield(&mut engine, 1, "propaganda");

    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("Propaganda opens attack payment");
    let transaction_id = engine
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending Propaganda payment")
        .transaction_id;
    activate_for_mana(&mut engine, first, 3);
    let activation = activate_for_mana(&mut engine, second, 3);
    let first_group = engine.state.players[0].restricted_mana[0].restriction_group_id;
    let second_group = engine.state.players[0].restricted_mana[1].restriction_group_id;
    assert_ne!(first_group, second_group);
    let pending_payment = activation.legal_by_player[&0]
        .pending_attack_declaration
        .as_ref()
        .unwrap_or_else(|| {
            panic!(
                "second Path activation response omitted the pending attack payment: {:#?}",
                activation.legal_by_player[&0]
            )
        });
    let undo_options = &pending_payment.mana_ability_undo_options;
    let undo_index = undo_options
        .iter()
        .find(|option| option.source_object_id == first)
        .unwrap_or_else(|| panic!("first Path undo option missing: {undo_options:#?}"))
        .activation_command_index;
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::UndoManaAbility(UndoManaAbility {
                    attack_transaction_id: transaction_id,
                    activation_command_index: undo_index,
                })),
            },
        )
        .expect("undo the first Path while retaining the later activation");

    assert_eq!(engine.state.players[0].restricted_mana.len(), 1);
    assert_eq!(
        engine.state.players[0].restricted_mana[0].restriction_group_id, second_group,
        "undo must not recycle the first Path's published group id for the second source"
    );
    assert!(!engine.state.objects[&first].tapped);
    assert!(engine.state.objects[&second].tapped);

    engine.state.players[0].mana_pool.colorless = 3;
    let pending = engine
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("pending Propaganda payment")
        .clone();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitAttackDeclaration(CommitAttackDeclaration {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    payment: Some(PaymentSelection {
                        expected_state_revision: engine.state.command_index,
                        mana: Some(PaymentMana {
                            c: 2,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .expect("ordinary mana pays Propaganda without consuming Path B's group");

    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let spell = inject_card_into_hand(&mut engine, 0, "appendage_amalgam");
    let slot = hand_index_for_card(&engine, 0, "appendage_amalgam");
    let mut stale = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: first_group,
        g: 1,
        ..Default::default()
    });
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("before stale selection");
    assert!(engine.apply_scenario_command(0, &stale).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), snapshot);
    assert_eq!(
        engine.state.objects[&spell].zone,
        tricerules_core::Zone::Hand
    );

    let mut accepted = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = accepted.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: second_group,
        g: 1,
        ..Default::default()
    });
    semantic::accepted(&mut engine, 0, &accepted);
    let triggers = path_scry_triggers(&engine);
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].source_permanent_id, Some(second));
}

#[test]
fn path_of_ancestry_ignores_a_face_down_commander_in_exile() {
    let mut engine = commander_mana_engine();
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    let commander = engine.state.players[0].command_zone.remove(0);
    engine.state.players[0].exile.push(commander);
    let object = engine.state.objects.get_mut(&commander).unwrap();
    object.zone = tricerules_core::Zone::Exile;
    object.face_down = true;
    let generation = engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_insert(0);
    *generation += 1;
    // The owner already declined the optional command-zone move for this generation.
    engine
        .state
        .commander_sba_checked_generations
        .insert(commander, *generation);
    engine.initial_response_batch();

    activate_for_mana(&mut engine, path, 3);
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path mana group")
        .restriction_group_id;
    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            c: 1,
            w: 1,
            u: 1,
            b: 1,
            ..Default::default()
        },
    );
    cast_with_path_mana(
        &mut engine,
        "atraxa,_praetors_voice",
        group,
        ManaSpendSelection {
            g: 1,
            ..Default::default()
        },
    );
    assert!(path_scry_triggers(&engine).is_empty());
}

#[test]
fn path_of_ancestry_checks_each_declared_commander_for_a_matching_type() {
    // This engine fixture exercises the type lookup across multiple declared commanders. Deck
    // pairing legality is validated elsewhere and is not part of this type-matching scenario.
    let mut engine = commander_mana_engine_with_commanders(&[
        "atraxa,_praetors_voice",
        "bello,_bard_of_the_brambles",
    ]);
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    // Bello's green identity contribution is the final option in W, U, B, R, G order.
    activate_for_mana(&mut engine, path, 4);
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path mana group")
        .restriction_group_id;
    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    cast_with_path_mana(
        &mut engine,
        "raccoon_rallier",
        group,
        ManaSpendSelection {
            g: 1,
            ..Default::default()
        },
    );
    assert_eq!(path_scry_triggers(&engine).len(), 1);
}

#[test]
fn transferred_path_mana_uses_the_path_activators_commander_and_controls_the_trigger() {
    let mut engine = commander_mana_engine_with_commanders(&["bello,_bard_of_the_brambles"]);
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    // Bello's color identity is red and green, so option 1 produces green.
    activate_for_mana(&mut engine, path, 1);
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path mana group")
        .restriction_group_id;

    // This models the CR 106.13 transfer of a mana unit and its rider (as with Drain Power).
    let source_contribution_index = engine.state.players[0]
        .restricted_mana
        .iter()
        .position(|contribution| contribution.restriction_group_id == group)
        .expect("restricted Path unit remains tied to its source");
    let transferred = engine.state.players[0]
        .restricted_mana
        .remove(source_contribution_index);
    engine.state.players[1].restricted_mana.push(transferred);
    super::helpers::give_mana(
        &mut engine,
        1,
        super::helpers::ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    // The Path controller's commander is a Raccoon; the spending player's commander is not.
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    let object = inject_card_into_hand(&mut engine, 1, "raccoon_rallier");
    let slot = hand_index_for_card(&engine, 1, "raccoon_rallier");
    let mut cast = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(command)) = cast.cmd.as_mut() else {
        unreachable!()
    };
    command.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group,
        g: 1,
        ..Default::default()
    });
    semantic::accepted(&mut engine, 1, &cast);

    let triggers = path_scry_triggers(&engine);
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].controller, 0);
    assert_eq!(triggers[0].source_permanent_id, Some(path));
    assert_eq!(
        engine.state.objects[&object].zone,
        tricerules_core::Zone::Stack
    );
}

#[test]
fn identical_path_sources_keep_separate_mana_groups_and_source_lki() {
    let mut engine = commander_mana_engine();
    let first = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    activate_for_mana(&mut engine, first, 3);
    activate_for_mana(&mut engine, second, 3);

    let first_group = engine.state.players[0].restricted_mana[0].restriction_group_id;
    let second_group = engine.state.players[0].restricted_mana[1].restriction_group_id;
    assert_ne!(first_group, second_group);
    let first_generation = engine
        .state
        .zone_change_generation
        .get(&first)
        .copied()
        .unwrap_or(0);
    move_battlefield_card_to_graveyard_fixture(&mut engine, 0, first);

    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            c: 1,
            w: 1,
            u: 1,
            b: 1,
            ..Default::default()
        },
    );
    cast_with_path_mana(
        &mut engine,
        "atraxa,_praetors_voice",
        first_group,
        ManaSpendSelection {
            g: 1,
            ..Default::default()
        },
    );

    let triggers = path_scry_triggers(&engine);
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].source_permanent_id, Some(first));
    assert_eq!(triggers[0].source_zone_change, first_generation);
    assert!(engine.state.zone_change_generation[&first] > first_generation);
    assert_eq!(engine.state.players[0].restricted_mana.len(), 1);
    assert_eq!(
        engine.state.players[0].restricted_mana[0].restriction_group_id,
        second_group
    );
}

#[test]
fn doubled_path_mana_creates_one_scry_trigger_per_mana_unit() {
    let mut engine = commander_mana_engine();
    inject_permanent_on_battlefield(&mut engine, 0, "mana_reflection");
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    // The commander color identity option order is W, U, B, R, G.
    activate_for_mana(&mut engine, path, 3);
    let contribution = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("doubled Path mana group");
    let group = contribution.restriction_group_id;
    assert_eq!(contribution.amount.g, 2);

    super::helpers::give_mana(
        &mut engine,
        0,
        super::helpers::ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    cast_with_path_mana(
        &mut engine,
        "chasm_skulker",
        group,
        ManaSpendSelection {
            g: 2,
            ..Default::default()
        },
    );
    answer_trigger_order_in_engine_order(&mut engine);
    let triggers = path_scry_triggers(&engine);
    assert_eq!(triggers.len(), 2);
    assert!(triggers
        .iter()
        .all(|trigger| trigger.source_permanent_id == Some(path)));

    resolve_scry_triggers_and_stack(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn path_mana_spent_on_an_activated_ability_does_not_scry() {
    let mut engine = commander_mana_engine();
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    let mind_stone = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    activate_for_mana(&mut engine, path, 3);
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Path mana group")
        .restriction_group_id;
    let mut activation = activate_ability_for(&engine, mind_stone, 1, vec![]);
    let Some(Cmd::ActivateAbility(command)) = activation.cmd.as_mut() else {
        unreachable!()
    };
    command.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group,
        g: 1,
        ..Default::default()
    });
    semantic::accepted(&mut engine, 0, &activation);

    assert!(path_scry_triggers(&engine).is_empty());
    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert_eq!(
        engine.state.objects[&mind_stone].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn undoing_a_path_mana_activation_removes_its_unspent_restricted_mana() {
    let mut engine = commander_mana_engine();
    let path = inject_permanent_on_battlefield(&mut engine, 0, "path_of_ancestry");
    activate_for_mana(&mut engine, path, 3);

    semantic::accepted(&mut engine, 0, &undo_mana_ability());
    assert!(!engine.state.objects[&path].tapped);
    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert!(path_scry_triggers(&engine).is_empty());
}

#[test]
fn forged_signet_color_choice_is_rejected_without_mutating_game_state() {
    let mut engine = commander_mana_engine();
    let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    let stack_len = engine.state.stack.len();
    let command_index = engine.state.command_index;
    let mut stale = activate_ability_for(&engine, signet, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;

    assert!(engine.apply_scenario_command(0, &stale).is_err());
    assert!(!engine.state.objects[&signet].tapped);
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    assert_eq!(engine.state.stack.len(), stack_len);
    assert_eq!(engine.state.command_index, command_index);

    let mut forged = activate_ability_for(&engine, signet, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = forged.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = 4; // Atraxa's WUBG identity has only four legal choices.

    assert!(engine.apply_scenario_command(0, &forged).is_err());
    assert!(!engine.state.objects[&signet].tapped);
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    assert_eq!(engine.state.stack.len(), stack_len);
    assert_eq!(engine.state.command_index, command_index);

    // A rejected forged option leaves the legal choices available and the source usable.
    activate_for_mana(&mut engine, signet, 2);
    assert!(engine.state.objects[&signet].tapped);
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
}
