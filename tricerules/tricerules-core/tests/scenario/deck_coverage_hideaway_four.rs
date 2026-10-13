//! Hideaway 4 identity, source-linked privacy, land play, and cast-and-return rules.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::registry;
use tricerules_cards::{primitives::*, Keyword, Layout};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CastSource, CostObjectRef, CostObjectRefs,
    CostSelection, LandSource, ResolutionChoiceDecision,
};

const MOSSWORT: &str = "mosswort_bridge";
const BUGGY: &str = "smugglers_buggy";

fn game(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[MOSSWORT, BUGGY, "opt", "grizzly_bears"]);
    let mut engine = GameEngine::new(
        registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_hideaway_look(engine: &mut GameEngine, chooser: i32, chosen_card: u32) {
    answer_trigger_order_in_engine_order(engine);
    pass_priority_round(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Hideaway offers a private top-card choice");
    assert_eq!(pending.deciding_player, chooser);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryLook);
    assert_eq!(pending.presentation.min, 1);
    assert_eq!(pending.presentation.max, 1);
    assert!(pending.presentation.candidates.contains(&chosen_card));
    engine
        .apply_scenario_command(chooser, &submit_resolution_choice(vec![chosen_card]))
        .expect("exile the selected Hideaway card face down");
    assert_eq!(engine.state.objects[&chosen_card].zone, Zone::Exile);
    assert!(engine.state.objects[&chosen_card].face_down);
}

fn put_hideaway_card_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    engine.state.players[player].library.push_front(object);
    object
}

fn one_creature_cost(object_id: u32, generation: u64) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id,
                zone_change_generation: generation,
            }],
        })),
    }
}

#[test]
fn mosswort_bridge_and_smugglers_buggy_register_their_exact_rules_shapes() {
    let bridge = registry::global()
        .get(MOSSWORT)
        .expect("Mosswort Bridge definition");
    assert_eq!(bridge.name, "Mosswort Bridge");
    assert_eq!(bridge.layout, Layout::Normal);
    let bridge_face = bridge.primary_face();
    assert_eq!(bridge_face.types, ["Land"]);
    assert!(matches!(
        bridge_face.static_abilities[0].definition,
        StaticAbilityDef::EntersTapped { .. }
    ));
    assert!(matches!(
        bridge_face.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Hideaway {
            count: 4,
            ref linked_exile_id,
        }] if linked_exile_id.as_str() == "exiled_cards"
    ));
    assert!(matches!(
        bridge_face.activated_abilities[0].effect.as_slice(),
        [SpellEffectKind::ProduceMana { .. }]
    ));
    assert!(matches!(
        bridge_face.activated_abilities[1].effect.as_slice(),
        [SpellEffectKind::ChooseResolutionBranch {
            optional: true,
            branches,
            ..
        }] if matches!(
            branches[0].requirement,
            ResolutionBranchRequirement::GameCondition(GameCondition::BattlefieldAggregate {
                aggregate: BattlefieldAggregate::TotalPower,
                min: Some(10),
                ..
            })
        ) && matches!(
            branches[0].effects.as_slice(),
            [SpellEffectKind::PlayLinkedExiledCard {
                play_kind: LinkedExiledCardPlayKind::Play,
                return_source_to_owner_hand_if_cast: false,
                ..
            }]
        )
    ));

    let buggy = registry::global()
        .get(BUGGY)
        .expect("Smuggler's Buggy definition");
    assert_eq!(buggy.name, "Smuggler's Buggy");
    assert_eq!(buggy.layout, Layout::Normal);
    let buggy_face = buggy.primary_face();
    assert_eq!(buggy_face.mana_cost.to_string(), "{4}");
    assert_eq!(buggy_face.types, ["Artifact", "Vehicle"]);
    assert_eq!((buggy_face.power, buggy_face.toughness), (Some(5), Some(5)));
    assert_eq!(buggy_face.triggered_abilities.len(), 2);
    assert_eq!(buggy_face.keywords, Vec::<Keyword>::new());
    assert!(matches!(
        buggy_face.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Hideaway { count: 4, .. }]
    ));
    assert!(matches!(
        buggy_face.triggered_abilities[1].effect.as_slice(),
        [SpellEffectKind::PlayLinkedExiledCard {
            play_kind: LinkedExiledCardPlayKind::Cast,
            return_source_to_owner_hand_if_cast: true,
            ..
        }]
    ));
    assert!(matches!(
        buggy_face.activated_abilities[0].costs.as_slice(),
        [AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::AggregateMinimum {
                minimum: 2,
                contribution: ObjectContributionKind::CurrentPower,
            },
            ..
        }]
    ));
}

#[test]
fn mosswort_bridge_checks_power_when_its_ability_resolves_then_plays_the_linked_land() {
    let mut engine = game(20_261_011);
    let bridge = inject_card_into_hand(&mut engine, 0, MOSSWORT);
    let entered = move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    assert_eq!(entered, bridge);
    assert!(engine.state.objects[&bridge].tapped);

    let linked_land = put_hideaway_card_on_top(&mut engine, 0, "mountain");
    resolve_hideaway_look(&mut engine, 0, linked_land);
    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_land].face_down);

    // Set up eight power when activating. The threshold is evaluated as the ability resolves,
    // so a fifth Bear entering while it is on the stack makes the linked play available.
    engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
    let initial_bears = (0..4)
        .map(|_| inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears"))
        .collect::<Vec<_>>();
    assert_eq!(
        initial_bears
            .iter()
            .map(|object| engine.characteristics(*object).unwrap().power.unwrap() as i64)
            .sum::<i64>(),
        8
    );
    engine.state.players[0].mana_pool.green = 1;
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, bridge, 1, Vec::new()))
        .expect("activate Mosswort Bridge with one green and tap it");
    let fifth_bear = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(
        initial_bears
            .iter()
            .copied()
            .chain([fifth_bear])
            .map(|object| engine.characteristics(object).unwrap().power.unwrap() as i64)
            .sum::<i64>(),
        10
    );

    pass_priority_round(&mut engine);
    let threshold_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Mosswort's qualifying branch asks whether to play the linked card");
    assert_eq!(
        threshold_choice.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose the qualifying optional branch");

    let linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("choose the exact linked Hideaway card");
    assert_eq!(
        linked_choice.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_eq!(linked_choice.presentation.candidates, [linked_land]);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![linked_land]))
        .expect("choose Mosswort Bridge's linked land");
    let exile_generation = engine.state.zone_change_generation[&linked_land];
    let land_action = RuledCommand {
        cmd: Some(Cmd::PlayLand(tricerules_proto::ruled::v1::PlayLand {
            resolution_prompt_id: 0,
            source: Some(LandSource {
                location: Some(
                    tricerules_proto::ruled::v1::land_source::Location::ExileObjectId(linked_land),
                ),
                expected_zone_change_generation: Some(exile_generation),
            }),
            face_index: 0,
        })),
    };
    let mut invalid_land_action = land_action.clone();
    let Some(Cmd::PlayLand(invalid_land)) = invalid_land_action.cmd.as_mut() else {
        unreachable!()
    };
    invalid_land.face_index = 1;
    assert!(engine
        .apply_scenario_command(0, &invalid_land_action)
        .is_err());
    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_land].face_down);

    let land_batch = engine
        .apply_scenario_command(0, &land_action)
        .expect("play the land during Mosswort's resolving ability");

    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&linked_land].controller, 0);
    assert!(
        !engine.state.objects[&linked_land].face_down,
        "CR 406.3a turns the face-down exiled card face up before it is played"
    );
    let characteristics = engine.characteristics(linked_land).unwrap();
    assert!(characteristics.has_type("Land"));
    assert!(!characteristics.is_creature());
    let reveal_index = land_batch
        .events
        .iter()
        .position(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                if reveal.source_zone == tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Exile as i32
                    && reveal.cards.iter().any(|card| card.object_id == linked_land && card.card_id == "mountain")
        ))
        .expect("the played card is publicly revealed from exile");
    let moved_index = land_batch
        .events
        .iter()
        .position(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == linked_land
                    && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32
                    && moved.card_id == "mountain"
                    && !moved.face_down
        ))
        .expect("the battlefield event identifies the face-up Mountain");
    assert!(
        reveal_index < moved_index,
        "reveal precedes battlefield entry"
    );
    assert_eq!(engine.state.lands_played_this_turn, 1);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn mosswort_bridge_does_not_offer_the_linked_card_below_ten_power() {
    let mut engine = game(20_261_013);
    let bridge = inject_card_into_hand(&mut engine, 0, MOSSWORT);
    move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    let linked_land = put_hideaway_card_on_top(&mut engine, 0, "forest");
    resolve_hideaway_look(&mut engine, 0, linked_land);

    engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
    for _ in 0..4 {
        inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    }
    engine.state.players[0].mana_pool.green = 1;
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, bridge, 1, Vec::new()))
        .expect("activate Mosswort Bridge below the power threshold");
    pass_priority_round(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_land].face_down);
    assert!(engine.state.objects[&bridge].tapped);
}

#[test]
fn mosswort_bridge_optional_branch_decline_keeps_the_linked_card_hidden() {
    let mut engine = game(20_261_015);
    let bridge = move_ready_to_battlefield(&mut engine, 0, MOSSWORT);
    let linked_land = put_hideaway_card_on_top(&mut engine, 0, "mountain");
    resolve_hideaway_look(&mut engine, 0, linked_land);
    assert!(engine.state.objects[&linked_land].face_down);

    engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
    for _ in 0..5 {
        inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    }
    engine.state.players[0].mana_pool.green = 1;
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, bridge, 1, Vec::new()))
        .expect("activate Mosswort Bridge with one green");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("qualifying ability offers its optional branch")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );

    let declined = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline to play the linked card");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&linked_land].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_land].face_down);
    assert!(!declined.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_land)
    )));
}

fn buggy_after_combat_hit(seed: u64) -> (GameEngine, u32, u32) {
    let mut engine = game(seed);
    let linked_card = put_hideaway_card_on_top(&mut engine, 0, "opt");
    let _buggy_in_hand = inject_card_into_hand(&mut engine, 0, BUGGY);
    let buggy = move_ready_to_battlefield(&mut engine, 0, BUGGY);
    assert!(!engine.state.objects[&buggy].tapped);
    resolve_hideaway_look(&mut engine, 0, linked_card);

    let crew = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let generation = engine
        .state
        .zone_change_generation
        .get(&crew)
        .copied()
        .unwrap_or(0);
    let mut crew_command = activate_ability_with_costs(
        buggy,
        0,
        Vec::new(),
        vec![one_creature_cost(crew, generation)],
    );
    let Some(Cmd::ActivateAbility(activation)) = crew_command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&buggy)
        .copied()
        .unwrap_or(0);
    engine
        .apply_scenario_command(0, &crew_command)
        .expect("crew Smuggler's Buggy with two power");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.characteristics(buggy).unwrap().is_creature());
    advance_to_combat_damage(&mut engine, buggy);
    assert_eq!(engine.state.players[1].life, 15);
    (engine, buggy, linked_card)
}

#[test]
fn smugglers_buggy_casts_its_linked_card_after_combat_damage_then_returns_itself() {
    let (mut engine, buggy, linked_card) = buggy_after_combat_hit(20_261_012);
    let source_generation = engine.state.zone_change_generation[&buggy];
    pass_priority_round(&mut engine);
    let linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Buggy's combat damage trigger offers its linked card");
    assert_eq!(
        linked_choice.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_eq!(linked_choice.presentation.candidates, [linked_card]);
    let cast_offer = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![linked_card]))
        .expect("choose the linked Opt");
    let cast_offer_logs = cast_offer
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log)) => {
                Some((log.text.clone(), log.visible_to_player_id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        cast_offer_logs.iter().any(
            |(text, visible_to)| text.contains("Face-down exiled card") && visible_to.is_none()
        ),
        "face-down cast offer logs: {cast_offer_logs:?}"
    );
    assert!(!cast_offer.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text.contains("cast Opt") && log.visible_to_player_id.is_none()
    )));

    let cast_action = engine.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == linked_card)
        .cloned()
        .expect("Buggy grants a current resolving cast permission");
    let linked_generation = engine.state.zone_change_generation[&linked_card];
    let cast = tricerules_proto::ruled::v1::CastSpell {
        cast_method: cast_action.cast_method,
        casting_permission_id: cast_action.casting_permission_id,
        source: Some(CastSource {
            expected_zone_change_generation: Some(linked_generation),
            location: Some(
                tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(linked_card),
            ),
        }),
        ..Default::default()
    };
    let cast_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice_with_cast(cast))
        .expect("cast Opt during Buggy's resolving trigger");

    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Stack);
    assert!(
        !engine.state.objects[&linked_card].face_down,
        "a successfully cast face-down exiled card is face up on the stack"
    );
    let reveal_index = cast_batch
        .events
        .iter()
        .position(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
                if reveal.source_zone == tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Exile as i32
                    && reveal.cards.iter().any(|card| card.object_id == linked_card && card.card_id == "opt")
        ))
        .expect("successful cast publicly reveals the card from Exile");
    let stack_index = cast_batch
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackPushed(stack))
                    if stack.object_id == linked_card && stack.card_id == "opt"
            )
        })
        .expect("successful cast publishes its printed StackPushed identity");
    assert!(reveal_index < stack_index, "reveal precedes StackPushed");
    assert_eq!(engine.state.objects[&buggy].zone, Zone::Hand);
    assert!(engine.state.zone_change_generation[&buggy] > source_generation);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn smugglers_buggy_decline_keeps_the_vehicle_and_linked_card_in_place() {
    let (mut engine, buggy, linked_card) = buggy_after_combat_hit(20_261_014);
    let source_generation = engine.state.zone_change_generation[&buggy];
    pass_priority_round(&mut engine);
    let linked_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Buggy's combat damage trigger offers its linked card");
    assert_eq!(
        linked_choice.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );

    let declined = engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline Buggy's optional linked cast");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_card].face_down);
    assert!(!declined.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_card)
    )));
    assert_eq!(engine.state.objects[&buggy].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&buggy],
        source_generation
    );
}

#[test]
fn cancelled_face_down_exile_spell_proposal_restores_its_hidden_exiled_card() {
    let mut engine = game(20_261_016);
    let linked_card = inject_card_into_hand(&mut engine, 0, "opt");
    engine.state.players[0]
        .hand
        .retain(|object_id| *object_id != linked_card);
    engine.state.players[0].exile.push(linked_card);
    let card = engine.state.objects.get_mut(&linked_card).unwrap();
    card.zone = Zone::Exile;
    card.face_down = true;
    let generation = engine
        .state
        .zone_change_generation
        .get(&linked_card)
        .copied()
        .unwrap_or(0);
    engine.state.active_exile_play_permissions.push(
        tricerules_core::state::ActiveExilePlayPermission {
            group_id: 20261016,
            player_id: 0,
            source_label: "test cast permission".into(),
            object_id: linked_card,
            zone_change_generation: generation,
            scope: tricerules_core::state::ExilePlayPermissionScope::CastCard,
            max_face_mana_value: None,
            cast_cost: tricerules_core::state::ExilePermissionCastCost::PrintedManaCost,
            origin: tricerules_core::state::ExilePlayPermissionOrigin::Effect,
            available_after_turn_instance: None,
            expires_at_cleanup_turn_instance: None,
        },
    );
    let cast_action = engine.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == linked_card)
        .cloned()
        .expect("the exact exile permission offers a cast action");
    let announcement = tricerules_proto::ruled::v1::SpellCastAnnouncement {
        source: Some(CastSource {
            expected_zone_change_generation: Some(generation),
            location: Some(
                tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(linked_card),
            ),
        }),
        cast_method: cast_action.cast_method,
        casting_permission_id: cast_action.casting_permission_id,
        ..Default::default()
    };
    let mut invalid_announcement = announcement.clone();
    invalid_announcement
        .source
        .as_mut()
        .unwrap()
        .expected_zone_change_generation = Some(generation.saturating_add(1));
    assert!(engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(
                    tricerules_proto::ruled::v1::BeginSpellCast {
                        announcement: Some(invalid_announcement),
                    },
                )),
            },
        )
        .is_err());
    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_card].face_down);

    let begin = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(
                    tricerules_proto::ruled::v1::BeginSpellCast {
                        announcement: Some(announcement),
                    },
                )),
            },
        )
        .expect("begin a valid linked spell-cast proposal");
    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Stack);
    assert!(engine.state.objects[&linked_card].face_down);
    assert!(!begin.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_card)
    )));

    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the valid announcement is awaiting payment")
        .transaction_id;
    let cancelled = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(
                    tricerules_proto::ruled::v1::CancelSpellCast { transaction_id },
                )),
            },
        )
        .expect("cancel the uncommitted proposal");
    assert_eq!(engine.state.objects[&linked_card].zone, Zone::Exile);
    assert!(engine.state.objects[&linked_card].face_down);
    assert!(!cancelled.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == linked_card)
    )));
}

fn submit_resolution_choice_with_cast(
    cast: tricerules_proto::ruled::v1::CastSpell,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                resolution_prompt_id: 0,
                decision: ResolutionChoiceDecision::CastSpell as i32,
                cast_spell: Some(cast),
                ..Default::default()
            },
        )),
    }
}

fn advance_to_combat_damage(engine: &mut GameEngine, attacker: u32) {
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("begin combat");
    for _ in 0..12 {
        if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
            break;
        }
        pass_priority_round(engine);
    }
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("attack with Smuggler's Buggy");
    for _ in 0..12 {
        if engine.state.turn_step == tricerules_core::TurnStep::CombatDamage {
            return;
        }
        pass_priority_round(engine);
    }
    panic!(
        "game did not reach combat damage: {:?}",
        engine.state.turn_step
    );
}
