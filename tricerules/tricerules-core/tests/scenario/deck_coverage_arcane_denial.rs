//! Actual-card coverage for Arcane Denial's target-controller capture and two independently
//! ordered delayed triggers at the next actual turn's upkeep.
use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_cards::primitives::{PlayerRecipient, SpellEffectKind, TriggerCondition};
use tricerules_cards::AbilityPresentation;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, PresentationPathKind, ResolutionChoiceDecision,
    ResolutionChoiceRequired, RuledCommand, SubmitResolutionChoice, TargetRef, TargetRefKind,
};

const PLAYER_IDS: [i32; 3] = [4, 9, 27];
const FOUR_PLAYER_IDS: [i32; 4] = [4, 9, 27, 35];

const SYNTHETIC_ARCANE_DELAYED_TAIL: &str = r#"
(
  id: "test_arcane_delayed_tail",
  name: "Test Arcane Delayed Tail",
  face_id: "test_arcane_delayed_tail",
  mana_cost: "{1}{U}",
  types: ["Instant"],
  spell_effect: [
    CounterTargetSpell(spell_filter: (), unless_controller_pays: None),
    CreateDelayedTrigger(
      subject: None,
      affected_player: Some(PreviousTargetedSpellController),
      ability: (
        ability_id: "delayed_01",
        presentation: Fallback,
        trigger: AtBeginningOfNextTurnUpkeep,
        effect: [],
        program: Some((body: (steps: [
          (id: "choose_count", kind: Choose(
            choice: "draw_count",
            chooser: AffectedPlayer,
            optional: false,
            branches: [
              (branch_id: "draw_zero", presentation: Fallback, cost: None, requirement: Always,
                body: (steps: [(id: "draw_zero", kind: Perform(Draw(
                  who: AffectedPlayer, count: 0)))], yields: [])),
              (branch_id: "draw_one", presentation: Fallback, cost: None, requirement: Always,
                body: (steps: [(id: "draw_one", kind: Perform(Draw(
                  who: AffectedPlayer, count: 1)))], yields: [])),
              (branch_id: "draw_two", presentation: Fallback, cost: None, requirement: Always,
                body: (steps: [(id: "draw_two", kind: Perform(Draw(
                  who: AffectedPlayer, count: 2)))], yields: [])),
            ],
            otherwise: (steps: [], yields: []),
          )),
          (id: "controller_life_tail", kind: Perform(GainLife(who: Controller, amount: 1))),
        ], yields: []), target_bindings: [])),
      ),
    ),
  ],
  targeting: Some((groups: [(
    min: 1,
    max: 1,
    prompt: "Choose target spell",
    effect_indices: [0],
  )])),
)
"#;

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        expected_zone_change_generation: None,
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: TargetRefKind::Stack as i32,
    }]
}

fn choose_branch(branch_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    }
}

fn set_library_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn prepare_arcane_denial_against_opt(seed: u64) -> (GameEngine, u32) {
    prepare_arcane_denial_against_opt_with_players(seed, &PLAYER_IDS)
}

fn prepare_arcane_denial_against_opt_with_players(
    seed: u64,
    player_ids: &[i32],
) -> (GameEngine, u32) {
    let decks = Some(vec![vec!["island".into(); 20]; player_ids.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        player_ids,
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    inject_card_into_hand(&mut engine, 1, "opt");
    inject_card_into_hand(&mut engine, 2, "arcane_denial");
    give_mana(
        &mut engine,
        player_ids[1],
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        player_ids[2],
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(player_ids[0], &pass())
        .expect("active player passes");
    let opt_slot = hand_index_for_card(&engine, 1, "opt");
    engine
        .apply_scenario_command(player_ids[1], &cast_spell(opt_slot, vec![]))
        .expect("target player casts Opt");
    let opt = engine.state.stack.last().expect("Opt on stack").id;
    engine
        .apply_scenario_command(player_ids[1], &pass())
        .expect("target player passes");
    let denial_slot = hand_index_for_card(&engine, 2, "arcane_denial");
    engine
        .apply_scenario_command(player_ids[2], &cast_spell(denial_slot, stack_target(opt)))
        .expect("third player casts Arcane Denial");
    (engine, opt)
}

fn cast_arcane_denial_against_opt(seed: u64) -> (GameEngine, u32) {
    cast_arcane_denial_against_opt_with_players(seed, &PLAYER_IDS)
}

fn cast_arcane_denial_against_opt_with_players(seed: u64, player_ids: &[i32]) -> (GameEngine, u32) {
    let (mut engine, opt) = prepare_arcane_denial_against_opt_with_players(seed, player_ids);
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty(), "Arcane Denial counters Opt");
    assert_eq!(engine.state.objects[&opt].zone, Zone::Graveyard);
    assert_eq!(engine.state.active_event_observers.len(), 2);
    assert!(engine.state.active_event_observers.iter().all(|observer| {
        matches!(
            observer.matcher,
            tricerules_core::state::EventObserverMatcher::AtBeginningOfNextTurnUpkeep {
                target_turn_instance: None,
                ..
            }
        )
    }));
    (engine, opt)
}

fn resolve_arcane_denial_against_uncounterable_eject(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![vec!["island".into(); 20]; PLAYER_IDS.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &PLAYER_IDS,
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 1, "eject");
    inject_card_into_hand(&mut engine, 2, "arcane_denial");
    give_mana(
        &mut engine,
        PLAYER_IDS[1],
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        PLAYER_IDS[2],
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(PLAYER_IDS[0], &pass())
        .expect("active player passes");
    let eject_slot = hand_index_for_card(&engine, 1, "eject");
    engine
        .apply_scenario_command(
            PLAYER_IDS[1],
            &cast_spell(eject_slot, target_object(creature)),
        )
        .expect("target player casts the uncounterable Eject");
    let eject = engine.state.stack.last().expect("Eject on stack").id;
    engine
        .apply_scenario_command(PLAYER_IDS[1], &pass())
        .expect("target player passes");
    let denial_slot = hand_index_for_card(&engine, 2, "arcane_denial");
    engine
        .apply_scenario_command(PLAYER_IDS[2], &cast_spell(denial_slot, stack_target(eject)))
        .expect("third player casts Arcane Denial at Eject");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "uncounterable Eject remains on the stack"
    );
    assert_eq!(engine.state.stack[0].id, eject);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    pass_priority_round(&mut engine);

    assert!(engine.state.stack.is_empty(), "Eject resolves normally");
    assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);
    assert_eq!(engine.state.active_event_observers.len(), 2);
    assert!(engine.state.active_event_observers.iter().all(|observer| {
        matches!(
            observer.matcher,
            tricerules_core::state::EventObserverMatcher::AtBeginningOfNextTurnUpkeep {
                target_turn_instance: None,
                ..
            }
        )
    }));
    (engine, creature)
}

fn order_arcane_delayed_triggers(
    engine: &mut GameEngine,
    controller: i32,
    target_resolves_first: bool,
) {
    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the Arcane Denial caster orders both simultaneous delayed triggers");
    assert_eq!(order.deciding_player, controller);
    assert_eq!(order.candidates.len(), 2);
    let target_trigger = order
        .candidates
        .iter()
        .find(|candidate| candidate.ability.program.is_some())
        .expect("target controller's draw-choice trigger");
    let own_trigger = order
        .candidates
        .iter()
        .find(|candidate| candidate.object_id != target_trigger.object_id)
        .expect("Arcane Denial controller's draw trigger");
    let first_to_place = if target_resolves_first {
        own_trigger.object_id
    } else {
        target_trigger.object_id
    };
    engine
        .apply_scenario_command(controller, &submit_trigger_order(first_to_place))
        .expect("choose the first delayed trigger to put on the stack");
    assert!(engine.state.pending_trigger_order.is_none());
    let top = engine
        .state
        .stack
        .last()
        .expect("both delayed abilities are on the stack");
    let top_is_target_trigger = top
        .triggered_ability
        .as_ref()
        .is_some_and(|ability| ability.program.is_some());
    assert_eq!(top_is_target_trigger, target_resolves_first);
    if top_is_target_trigger {
        assert_eq!(top.trigger_context.affected_player, Some(PLAYER_IDS[1]));
    }
}

fn resolve_arcane_delayed_triggers(
    engine: &mut GameEngine,
    draw_count: u32,
    target_left_game: bool,
) {
    let mut target_choice_seen = false;
    while !engine.state.stack.is_empty() || engine.state.pending_resolution.is_some() {
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            assert_eq!(
                pending.presentation.choice_kind,
                ChoiceKind::ResolutionBranch
            );
            let deciding_player = if target_left_game {
                PLAYER_IDS[0]
            } else {
                PLAYER_IDS[1]
            };
            assert_eq!(pending.deciding_player, deciding_player);
            target_choice_seen = true;
            assert!(
                engine
                    .apply_scenario_command(PLAYER_IDS[2], &choose_branch(0))
                    .is_err(),
                "the trigger controller cannot choose the draw count"
            );
            engine
                .apply_scenario_command(deciding_player, &choose_branch(draw_count))
                .expect("the target player or their rules-selected delegate chooses 0, 1, or 2");
        } else {
            pass_priority_round(engine);
        }
    }
    assert!(
        target_choice_seen,
        "the delayed draw choice is delegated when its chooser left"
    );
}

#[test]
fn arcane_denial_is_admitted_with_both_delayed_effects_and_exact_choice_ownership() {
    let face = tricerules_cards::registry::global()
        .get("arcane_denial")
        .expect("Arcane Denial is admitted as a complete card")
        .primary_face();

    assert_eq!(face.name, "Arcane Denial");
    assert_eq!(face.types, ["Instant"]);
    assert!(matches!(
        face.spell_effect.as_slice(),
        [
            SpellEffectKind::CounterTargetSpell { .. },
            SpellEffectKind::CreateDelayedTrigger {
                subject: None,
                affected_player: Some(PlayerRecipient::PreviousTargetedSpellController),
                ability: first,
            },
            SpellEffectKind::CreateDelayedTrigger {
                subject: None,
                affected_player: None,
                ability: second,
            }
        ] if first.presentation == AbilityPresentation::OracleLines(vec![1])
            && second.presentation == AbilityPresentation::OracleLines(vec![2])
            && first.trigger == TriggerCondition::AtBeginningOfNextTurnUpkeep
            && second.trigger == TriggerCondition::AtBeginningOfNextTurnUpkeep
            && first.effect.is_empty()
            && first.program.is_some()
            && matches!(
                second.effect.as_slice(),
                [SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    ..
                }]
            )
    ));
}

#[test]
fn arcane_denial_compiles_delayed_01_under_its_exact_face_owner() {
    let owner = tricerules_cards::ProgramOwnerId::ability(tricerules_cards::AbilityDefinitionId {
        card_id: "arcane_denial".into(),
        face_id: tricerules_cards::CardFaceId::new("arcane_denial").unwrap(),
        ability_path: vec![tricerules_cards::AbilityId::new("delayed_01").unwrap()],
    });
    let compiled = tricerules_cards::registry::global()
        .compiled_program_for_owner(&owner)
        .expect("Arcane Denial's delayed_01 child owns its face-local Program");

    assert_eq!(compiled.owner, owner);
    assert_eq!(
        compiled.required_inputs,
        vec![tricerules_cards::primitives::ProgramRequiredInput::TriggerAffectedPlayer]
    );

    let [instruction] = compiled.body.steps.as_slice() else {
        panic!("delayed_01 has exactly one choose-count Program instruction");
    };
    let tricerules_cards::primitives::CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
    } = &instruction.kind
    else {
        panic!("delayed_01 starts by choosing the target's draw count");
    };
    assert_eq!(
        *choice,
        tricerules_cards::ChoiceId::new("draw_count").unwrap()
    );
    assert_eq!(*chooser, PlayerRecipient::AffectedPlayer);
    assert!(!optional);
    assert!(otherwise.steps.is_empty());

    let expected_branches = [
        ("draw_zero", 0, "The affected player draws no cards."),
        ("draw_one", 1, "The affected player draws a card."),
        ("draw_two", 2, "The affected player draws 2 cards."),
    ];
    assert_eq!(branches.len(), expected_branches.len());
    for (branch, (branch_id, draw_count, fallback_label)) in branches.iter().zip(expected_branches)
    {
        assert_eq!(
            branch.branch_id,
            tricerules_cards::ChoiceId::new(branch_id).unwrap()
        );
        assert_eq!(
            branch.presentation,
            AbilityPresentation::OracleLines(vec![1]),
        );
        assert_eq!(
            branch.cost,
            tricerules_cards::primitives::ResolutionCost::None
        );
        assert_eq!(
            branch.requirement,
            tricerules_cards::primitives::CompiledChooseRequirement::Always
        );
        assert_eq!(branch.fallback_label(), fallback_label);
        let [step] = branch.body.steps.as_slice() else {
            panic!("each choice branch contains exactly one draw operation");
        };
        assert!(matches!(
            &step.kind,
            tricerules_cards::primitives::CompiledInstructionKind::Perform(effect)
                if matches!(
                    effect.as_ref(),
                    SpellEffectKind::Draw {
                        who: PlayerRecipient::AffectedPlayer,
                        count: tricerules_cards::primitives::Amount::Fixed(actual),
                    } if *actual == draw_count
                )
        ));
    }

    let delayed_02_owner =
        tricerules_cards::ProgramOwnerId::ability(tricerules_cards::AbilityDefinitionId {
            card_id: "arcane_denial".into(),
            face_id: tricerules_cards::CardFaceId::new("arcane_denial").unwrap(),
            ability_path: vec![tricerules_cards::AbilityId::new("delayed_02").unwrap()],
        });
    assert!(
        tricerules_cards::registry::global()
            .compiled_program_for_owner(&delayed_02_owner)
            .is_none(),
        "delayed_02 remains the existing non-Program trigger"
    );
}

#[test]
fn arcane_denial_discards_delayed_triggers_when_the_caster_leaves_before_upkeep() {
    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84270, &ids);
    assert_eq!(engine.state.active_event_observers.len(), 2);

    engine
        .apply_scenario_command(ids[2], &concede())
        .expect("Arcane Denial's controller leaves before the next upkeep");

    assert!(
        engine.state.active_event_observers.is_empty(),
        "a delayed triggered ability controlled by a departed player cannot later be put on the stack"
    );
    end_active_turn(&mut engine, ids[0]);
    assert_eq!(engine.state.active_player_id(), ids[1]);
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn arcane_denial_discards_staged_triggers_when_the_caster_leaves_before_placement() {
    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84271, &ids);
    end_active_turn(&mut engine, ids[0]);
    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("Arcane Denial's simultaneous delayed triggers await the caster's order");
    assert_eq!(order.deciding_player, ids[2]);
    assert_eq!(order.candidates.len(), 2);

    engine
        .apply_scenario_command(ids[2], &concede())
        .expect("the delayed-trigger controller leaves before placing either ability");

    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.staged_trigger_groups.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn arcane_denial_orders_both_triggers_and_the_target_chooses_zero_one_or_two() {
    for target_resolves_first in [false, true] {
        for draw_count in 0..=2 {
            let seed = 84200 + u64::from(target_resolves_first) * 10 + u64::from(draw_count);
            let (mut engine, _) = cast_arcane_denial_against_opt(seed);
            let target_cards = set_library_top(&mut engine, 1, &["forest", "mountain", "plains"]);
            let controller_cards =
                set_library_top(&mut engine, 2, &["grizzly_bears", "storm_crow"]);
            let target_hand_before = engine.state.players[1].hand.len();
            let controller_hand_before = engine.state.players[2].hand.len();

            end_active_turn(&mut engine, PLAYER_IDS[0]);
            assert_eq!(engine.state.active_player_id(), PLAYER_IDS[1]);
            assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
            order_arcane_delayed_triggers(&mut engine, PLAYER_IDS[2], target_resolves_first);
            resolve_arcane_delayed_triggers(&mut engine, draw_count, false);

            assert_eq!(
                engine.state.players[1].hand.len(),
                target_hand_before + draw_count as usize,
            );
            assert_eq!(
                engine.state.players[2].hand.len(),
                controller_hand_before + 1,
            );
            assert!(target_cards[..draw_count as usize]
                .iter()
                .all(|object_id| engine.state.players[1].hand.contains(object_id)));
            assert!(engine.state.players[2].hand.contains(&controller_cards[0]));
        }
    }
}

#[test]
fn arcane_denial_still_uses_an_uncounterable_targets_controller_for_the_delayed_draw() {
    let (mut engine, creature) = resolve_arcane_denial_against_uncounterable_eject(84240);
    assert_eq!(engine.state.objects[&creature].owner, PLAYER_IDS[1]);
    let target_hand_before = engine.state.players[1].hand.len();
    let controller_hand_before = engine.state.players[2].hand.len();
    let target_cards = set_library_top(&mut engine, 1, &["forest", "mountain", "plains"]);
    let controller_cards = set_library_top(&mut engine, 2, &["grizzly_bears", "storm_crow"]);

    end_active_turn(&mut engine, PLAYER_IDS[0]);
    assert_eq!(engine.state.active_player_id(), PLAYER_IDS[1]);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    order_arcane_delayed_triggers(&mut engine, PLAYER_IDS[2], true);
    resolve_arcane_delayed_triggers(&mut engine, 2, false);

    assert_eq!(engine.state.players[1].hand.len(), target_hand_before + 2);
    assert_eq!(
        engine.state.players[2].hand.len(),
        controller_hand_before + 1
    );
    assert!(target_cards[..2]
        .iter()
        .all(|object_id| engine.state.players[1].hand.contains(object_id)));
    assert!(engine.state.players[2].hand.contains(&controller_cards[0]));
}

#[test]
fn arcane_denial_with_all_targets_illegal_creates_no_delayed_triggers() {
    let (mut engine, opt) = prepare_arcane_denial_against_opt(84241);
    let denial = engine
        .state
        .stack
        .last()
        .expect("Arcane Denial on stack")
        .id;
    engine.state.stack.retain(|item| item.id != opt);
    engine.state.players[1].graveyard.push(opt);
    engine.state.objects.get_mut(&opt).expect("Opt object").zone = Zone::Graveyard;
    *engine.state.zone_change_generation.entry(opt).or_default() += 1;

    pass_priority_round(&mut engine);

    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&denial].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opt].zone, Zone::Graveyard);
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn arcane_delayed_program_missing_affected_player_rejects_resolution_atomically() {
    let [a, _b, controller] = PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt(84242);
    for observer in &mut engine.state.active_event_observers {
        if let tricerules_core::state::EventObserverPayload::StageDelayedTrigger(payload) =
            &mut observer.payload
        {
            if payload.program_definition.is_some() {
                payload.affected_player = None;
            }
        }
    }

    end_active_turn(&mut engine, a);
    assert_eq!(engine.state.active_player_id(), PLAYER_IDS[1]);
    runtime_proof_order_child_first(&mut engine, controller);
    let child = engine
        .state
        .stack
        .last()
        .expect("delayed Program child on stack");
    assert_eq!(
        child.ability_program_owner(),
        Some(runtime_proof_child_owner())
    );
    assert_eq!(child.trigger_context.affected_player, None);

    let active_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let remaining_passes = active_players
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("consistent priority pass count");
    for _ in 1..remaining_passes {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority before the Program child resolves");
    }
    let actor = engine.state.priority_player_id();
    let before_rejected_resolution = engine.diagnostic_snapshot().unwrap();
    let error = engine
        .apply_scenario_command(actor, &pass())
        .expect_err("missing TriggerAffectedPlayer rejects the staged child before offering it");
    assert!(
        matches!(error, tricerules_core::EngineError::ProgramInvariant(_)),
        "the staged input failure is a ProgramInvariant: {error:?}"
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_rejected_resolution,
        "a staged Program missing its required affected player fails atomically"
    );
}

#[test]
fn arcane_denials_target_player_leaving_before_upkeep_does_not_stall_the_controllers_draw() {
    let (mut engine, _) = cast_arcane_denial_against_opt(84230);
    engine
        .apply_scenario_command(PLAYER_IDS[1], &concede())
        .expect("target spell controller concedes before the upkeep");
    let controller_hand_before = engine.state.players[2].hand.len();
    let controller_cards = set_library_top(&mut engine, 2, &["grizzly_bears", "storm_crow"]);

    end_active_turn(&mut engine, PLAYER_IDS[0]);
    assert_eq!(engine.state.active_player_id(), PLAYER_IDS[2]);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    order_arcane_delayed_triggers(&mut engine, PLAYER_IDS[2], true);
    resolve_arcane_delayed_triggers(&mut engine, 0, true);

    assert_eq!(
        engine.state.players[2].hand.len(),
        controller_hand_before + 1
    );
    assert!(engine.state.players[2].hand.contains(&controller_cards[0]));
}

fn resolve_target_draw_trigger(
    engine: &mut GameEngine,
    controller: i32,
) -> ResolutionChoiceRequired {
    let batch = runtime_proof_pass_round_with_batch(engine);
    let choice =
        find_resolution_choice(&batch).expect("the target-player draw trigger asks for a delegate");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the target-player draw trigger asks for a delegate");
    assert_eq!(pending.deciding_player, controller);
    assert!(matches!(
        pending.continuation,
        tricerules_core::state::ResolutionContinuation::ProgramChoose { .. }
    ));
    assert_eq!(choice.deciding_player_id, controller);
    assert_eq!(choice.choice_kind, ChoiceKind::ResolutionBranch as i32);
    choice
}

#[test]
fn arcane_denial_refreshes_delegate_candidates_without_reusing_stale_indices() {
    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84250, &ids);
    engine
        .apply_scenario_command(ids[1], &concede())
        .expect("target spell controller leaves before the upkeep");
    end_active_turn(&mut engine, ids[0]);
    assert_eq!(engine.state.active_player_id(), ids[2]);
    order_arcane_delayed_triggers(&mut engine, PLAYER_IDS[2], true);
    let initial_choice = resolve_target_draw_trigger(&mut engine, ids[2]);

    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert!(matches!(
        pending.continuation,
        tricerules_core::state::ResolutionContinuation::ProgramChoose { .. }
    ));
    assert_eq!(initial_choice.resolution_branches.len(), 2);
    assert!(initial_choice
        .resolution_branches
        .iter()
        .all(|branch| branch.selectable));
    assert_eq!(
        initial_choice
            .resolution_branches
            .iter()
            .map(|branch| branch.label.as_str())
            .collect::<Vec<_>>(),
        ["P4 makes the choice", "P35 makes the choice"]
    );
    assert!(engine
        .apply_scenario_command(ids[0], &choose_branch(0))
        .is_err());

    let events = engine
        .apply_scenario_command(ids[0], &concede())
        .expect("one delegate candidate leaves while the controller is choosing");
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, ids[2]);
    let choice = events
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) => Some(choice),
            _ => None,
        })
        .expect("the delegate prompt is refreshed");
    assert_eq!(choice.resolution_branches.len(), 2);
    assert!(!choice.resolution_branches[0].selectable);
    assert!(choice.resolution_branches[1].selectable);
    assert_eq!(
        choice.resolution_branches[1]
            .presentation
            .as_ref()
            .unwrap()
            .fallback_text,
        "P35 makes the choice"
    );
    assert!(engine
        .apply_scenario_command(ids[2], &choose_branch(0))
        .is_err());
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        ids[2],
        "a stale candidate index cannot change the pending chooser"
    );

    engine
        .apply_scenario_command(ids[2], &choose_branch(1))
        .expect("the preserved index selects the remaining delegate");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        ids[3]
    );
    assert!(engine
        .apply_scenario_command(ids[2], &choose_branch(2))
        .is_err());
    engine
        .apply_scenario_command(ids[3], &choose_branch(2))
        .expect("the selected delegate chooses the count for the departed target");

    let target_hand_before = engine.state.players[1].hand.len();
    let controller_hand_before = engine.state.players[2].hand.len();
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[1].hand.len(), target_hand_before);
    assert_eq!(
        engine.state.players[2].hand.len(),
        controller_hand_before + 1
    );
}

#[test]
fn arcane_denial_delegate_choice_replays_accepted_commands_from_identical_state() {
    use prost::Message;

    type RecordedCommand = (
        i32,
        Vec<u8>,
        tricerules_proto::ruled::v1::RuledEventBatch,
        serde_json::Value,
    );

    fn record_command(
        engine: &mut GameEngine,
        log: &mut Vec<RecordedCommand>,
        actor: i32,
        command: RuledCommand,
    ) {
        let batch = engine
            .apply_scenario_command(actor, &command)
            .expect("record only accepted commands");
        log.push((
            actor,
            command.encode_to_vec(),
            batch,
            engine.diagnostic_snapshot().unwrap(),
        ));
    }

    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84272, &ids);
    let (mut replay, _) = cast_arcane_denial_against_opt_with_players(84272, &ids);
    engine
        .apply_scenario_command(ids[1], &concede())
        .expect("target player leaves before the delayed upkeep");
    replay
        .apply_scenario_command(ids[1], &concede())
        .expect("replay the target player's concession");
    end_active_turn(&mut engine, ids[0]);
    end_active_turn(&mut replay, ids[0]);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    assert_eq!(engine.state.active_player_id(), ids[2]);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    let mut accepted = Vec::new();

    let order = engine.state.pending_trigger_order.as_ref().unwrap();
    let target_trigger = order
        .candidates
        .iter()
        .find(|candidate| candidate.ability.program.is_some())
        .expect("the target draw-choice trigger");
    let own_trigger = order
        .candidates
        .iter()
        .find(|candidate| candidate.object_id != target_trigger.object_id)
        .expect("the caster's draw trigger");
    let own_trigger_object_id = own_trigger.object_id;
    record_command(
        &mut engine,
        &mut accepted,
        ids[2],
        submit_trigger_order(own_trigger_object_id),
    );

    let live_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let passes_needed = live_players - engine.state.passes_since_stack_change as usize;
    for _ in 0..passes_needed {
        let actor = engine.state.priority_player_id();
        record_command(&mut engine, &mut accepted, actor, pass());
    }
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        tricerules_core::state::ResolutionContinuation::ProgramChoose { .. }
    ));
    record_command(&mut engine, &mut accepted, ids[2], choose_branch(0));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        ids[0]
    );
    record_command(&mut engine, &mut accepted, ids[0], choose_branch(1));

    for (actor, encoded, expected_batch, expected_state) in accepted {
        let command = RuledCommand::decode(encoded.as_slice()).unwrap();
        assert_eq!(
            replay.apply_scenario_command(actor, &command).unwrap(),
            expected_batch
        );
        assert_eq!(replay.diagnostic_snapshot().unwrap(), expected_state);
    }
}

#[test]
fn arcane_denial_reassigns_when_the_selected_delegate_leaves_during_their_choice() {
    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84251, &ids);
    engine
        .apply_scenario_command(ids[1], &concede())
        .expect("target spell controller leaves before the upkeep");
    end_active_turn(&mut engine, ids[0]);
    order_arcane_delayed_triggers(&mut engine, PLAYER_IDS[2], true);
    resolve_target_draw_trigger(&mut engine, ids[2]);
    let delegated = engine
        .apply_scenario_command(ids[2], &choose_branch(0))
        .expect("controller delegates to the first surviving opponent");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        ids[0]
    );
    let stale_delegate_offer = delegated
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) => Some(choice.clone()),
            _ => None,
        })
        .expect("the selected delegate receives the actual branch offer");
    assert_eq!(stale_delegate_offer.deciding_player_id, ids[0]);
    assert_ne!(stale_delegate_offer.resolution_prompt_id, 0);

    let refreshed = engine
        .apply_scenario_command(ids[0], &concede())
        .expect("selected delegate leaves before answering the branch choice");
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, ids[3]);
    assert!(refreshed.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.deciding_player_id == ids[3]
                && choice.resolution_branches.len() == 3
    )));
    assert!(matches!(
        pending.continuation,
        tricerules_core::state::ResolutionContinuation::ProgramChoose { .. }
    ));
    let reassigned_offer = refreshed
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) => Some(choice.clone()),
            _ => None,
        })
        .expect("the sole eligible delegate receives a fresh branch offer");
    assert_eq!(reassigned_offer.deciding_player_id, ids[3]);
    assert_ne!(reassigned_offer.resolution_prompt_id, 0);

    let stale_answer = runtime_proof_answer_branch(
        &stale_delegate_offer,
        &stale_delegate_offer.resolution_branches[1].label,
    );
    let after_departure = engine.diagnostic_snapshot().unwrap();
    for actor in [ids[0], ids[2]] {
        assert!(
            engine.apply_scenario_command(actor, &stale_answer).is_err(),
            "departed delegate and live controller cannot answer the stale delegate prompt"
        );
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            after_departure,
            "a stale or wrong-actor choice rejection is atomic"
        );
    }

    let accepted_answer = runtime_proof_answer_branch(
        &reassigned_offer,
        &reassigned_offer.resolution_branches[1].label,
    );
    let before_answer = engine.diagnostic_snapshot().unwrap();
    engine
        .apply_scenario_command(ids[3], &accepted_answer)
        .expect("the remaining eligible player receives the refreshed branch prompt");
    let after_answer = engine.diagnostic_snapshot().unwrap();
    assert_ne!(
        after_answer, before_answer,
        "the current delegate's choice resolves once"
    );
    assert!(
        engine
            .apply_scenario_command(ids[3], &accepted_answer)
            .is_err(),
        "the accepted delegate prompt cannot be replayed"
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        after_answer,
        "a replayed branch choice is atomic"
    );
}

#[test]
fn narset_keeps_arcane_denials_zero_and_one_draw_options_but_hides_two() {
    let (mut engine, _) = cast_arcane_denial_against_opt(84280);
    inject_card_into_hand(&mut engine, 0, "narset,_parter_of_veils");
    move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    let target_hand_before = engine.state.players[1].hand.len();

    end_active_turn(&mut engine, PLAYER_IDS[0]);
    assert_eq!(engine.state.active_player_id(), PLAYER_IDS[1]);
    order_arcane_delayed_triggers(&mut engine, PLAYER_IDS[2], true);

    let choice = loop {
        if engine.state.pending_resolution.is_some() {
            break None;
        }
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority until the target draw choice is published");
        if let Some(choice) = batch.events.iter().find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) => Some(choice.clone()),
            _ => None,
        }) {
            break Some(choice);
        }
    }
    .expect("Arcane Denial publishes its draw-count choice");

    assert_eq!(choice.deciding_player_id, PLAYER_IDS[1]);
    assert_eq!(
        choice
            .resolution_branches
            .iter()
            .map(|branch| branch.branch_index)
            .collect::<Vec<_>>(),
        [0, 1],
        "Draw0 remains available, Draw1 fits Narset's limit, and Draw2 does not"
    );
    assert_eq!(
        choice.resolution_branches[0].label,
        "The affected player draws no cards."
    );
    assert_eq!(
        choice.resolution_branches[1].label,
        "The affected player draws a card."
    );

    engine
        .state
        .turn_history
        .current
        .player_mut(PLAYER_IDS[1])
        .cards_drawn = 1;
    let pending_after_limit_changed = format!("{:?}", engine.state.pending_resolution);
    assert!(
        engine
            .apply_scenario_command(PLAYER_IDS[1], &choose_branch(1))
            .is_err(),
        "the previously published one-card branch is rejected after the cap is used"
    );
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_after_limit_changed
    );
    assert_eq!(engine.state.players[1].hand.len(), target_hand_before);
    engine
        .state
        .turn_history
        .current
        .player_mut(PLAYER_IDS[1])
        .cards_drawn = 0;

    engine
        .apply_scenario_command(PLAYER_IDS[1], &choose_branch(1))
        .expect("the affected player selects the legal one-card branch");
    assert_eq!(engine.state.players[1].hand.len(), target_hand_before + 1);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(PLAYER_IDS[1])
            .cards_drawn,
        1
    );
}

fn runtime_proof_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![vec!["island".into(); 20]; PLAYER_IDS.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &PLAYER_IDS,
        20,
        decks,
        true,
    )
    .expect("new three-player runtime-proof game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn runtime_proof_answer_objects(
    offer: &ResolutionChoiceRequired,
    chosen_object_ids: Vec<u32>,
) -> RuledCommand {
    assert_ne!(
        offer.resolution_prompt_id, 0,
        "use the actual offered prompt ID"
    );
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: offer.resolution_prompt_id,
            chosen_object_ids,
            ..Default::default()
        })),
    }
}

fn runtime_proof_answer_players(
    offer: &ResolutionChoiceRequired,
    chosen_player_ids: Vec<i32>,
) -> RuledCommand {
    assert_ne!(
        offer.resolution_prompt_id, 0,
        "use the actual offered prompt ID"
    );
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: offer.resolution_prompt_id,
            chosen_player_ids,
            ..Default::default()
        })),
    }
}

fn runtime_proof_answer_branch(offer: &ResolutionChoiceRequired, label: &str) -> RuledCommand {
    assert_ne!(
        offer.resolution_prompt_id, 0,
        "use the actual offered prompt ID"
    );
    let branch = offer
        .resolution_branches
        .iter()
        .find(|branch| branch.label == label)
        .unwrap_or_else(|| panic!("offered branch {label:?} is missing"));
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: offer.resolution_prompt_id,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch.branch_index,
            ..Default::default()
        })),
    }
}

fn runtime_proof_reject_other_actors_atomically(
    engine: &mut GameEngine,
    offer: &ResolutionChoiceRequired,
    label: &str,
    other_actors: &[i32],
) {
    let command = runtime_proof_answer_branch(offer, label);
    let before = engine
        .diagnostic_snapshot()
        .expect("snapshot before unauthorized choice attempts");
    for actor in other_actors {
        assert!(
            engine.apply_scenario_command(*actor, &command).is_err(),
            "player {actor} cannot answer another player's offered choice"
        );
        assert_eq!(
            engine
                .diagnostic_snapshot()
                .expect("snapshot after rejected choice attempt"),
            before,
            "a rejected choice attempt is atomic"
        );
    }
}

fn runtime_proof_assert_arcane_branch_presentation(
    offer: &ResolutionChoiceRequired,
    label: &str,
    branch_id: &str,
) {
    let branch = offer
        .resolution_branches
        .iter()
        .find(|branch| branch.label == label)
        .unwrap_or_else(|| panic!("offered branch {label:?} is missing"));
    let presentation = branch
        .presentation
        .as_ref()
        .expect("the actual branch offer carries its presentation reference");
    assert_eq!(presentation.card_id, "arcane_denial");
    assert_eq!(presentation.face_id, "arcane_denial");
    assert_eq!(presentation.oracle_line_indices, [1]);
    assert_eq!(
        presentation
            .path
            .iter()
            .map(|component| component.kind)
            .collect::<Vec<_>>(),
        [
            PresentationPathKind::Spell as i32,
            PresentationPathKind::Ability as i32,
            PresentationPathKind::ResolutionBranch as i32,
        ],
        "the offered branch follows FaceSpell -> delayed_01 -> resolution branch"
    );
    assert_eq!(
        presentation
            .path
            .iter()
            .map(|component| component.id.as_str())
            .collect::<Vec<_>>(),
        ["spell", "delayed_01", branch_id]
    );
}

fn runtime_proof_assert_all_arcane_branch_presentations(offer: &ResolutionChoiceRequired) {
    let expected = [
        ("draw_zero", "The affected player draws no cards."),
        ("draw_one", "The affected player draws a card."),
        ("draw_two", "The affected player draws 2 cards."),
    ];
    assert_eq!(offer.resolution_branches.len(), expected.len());
    for (branch_id, label) in expected {
        runtime_proof_assert_arcane_branch_presentation(offer, label, branch_id);
    }
}

fn runtime_proof_pass_to(engine: &mut GameEngine, desired_player_id: i32) {
    let live_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    for _ in 0..live_players {
        let actor = engine.state.priority_player_id();
        if actor == desired_player_id {
            return;
        }
        engine
            .apply_scenario_command(actor, &pass())
            .expect("the currently offered priority holder passes");
    }
    assert_eq!(
        engine.state.priority_player_id(),
        desired_player_id,
        "desired caster receives priority within one ordinary priority rotation"
    );
}

fn runtime_proof_pass_round_with_batch(engine: &mut GameEngine) -> RuledEventBatch {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("consistent priority pass count");
    let mut last = RuledEventBatch::default();
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        last = engine
            .apply_scenario_command(actor, &pass())
            .expect("priority holder passes");
    }
    last
}

fn runtime_proof_child_definition() -> tricerules_cards::AbilityDefinitionId {
    tricerules_cards::AbilityDefinitionId {
        card_id: "arcane_denial".into(),
        face_id: tricerules_cards::CardFaceId::new("arcane_denial").unwrap(),
        ability_path: vec![tricerules_cards::AbilityId::new("delayed_01").unwrap()],
    }
}

fn runtime_proof_child_owner() -> tricerules_cards::ProgramOwnerId {
    tricerules_cards::ProgramOwnerId::ability(runtime_proof_child_definition())
}

fn runtime_proof_assert_delayed_pair(
    engine: &GameEngine,
    expected_source_id: u32,
    controller: i32,
    affected_player: i32,
) {
    use tricerules_core::state::EventObserverPayload;

    let payloads = engine
        .state
        .active_event_observers
        .iter()
        .filter_map(|observer| match &observer.payload {
            EventObserverPayload::StageDelayedTrigger(payload) => Some(payload.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(payloads.len(), 2, "one Arcane Denial pair is registered");
    assert!(payloads
        .iter()
        .all(|payload| payload.card_id == "arcane_denial"
            && payload.controller == controller
            && payload.source.object_id == expected_source_id));

    let expected_definition = runtime_proof_child_definition();
    let program = payloads
        .iter()
        .find(|payload| payload.program_definition.as_ref() == Some(&expected_definition))
        .expect("the delayed_01 payload carries Arcane Denial's exact child definition");
    assert_eq!(program.affected_player, Some(affected_player));
    assert_eq!(
        payloads
            .iter()
            .filter(|payload| payload.program_definition.is_some())
            .count(),
        1,
        "delayed_02 remains the independent legacy controller draw"
    );
    let controller_draw = payloads
        .iter()
        .find(|payload| payload.program_definition.is_none())
        .expect("the delayed_02 payload remains independent");
    assert_eq!(controller_draw.affected_player, None);
}

fn runtime_proof_order_child_first(engine: &mut GameEngine, controller: i32) {
    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("both delayed triggers need their controller's ordering choice");
    let child = order
        .candidates
        .iter()
        .find(|candidate| candidate.ability.program.is_some())
        .expect("the Program child is an offered delayed trigger");
    let child_id = child.object_id;
    let delayed_02_id = order
        .candidates
        .iter()
        .find(|candidate| candidate.object_id != child_id)
        .expect("the independent delayed_02 is the other candidate")
        .object_id;
    engine
        .apply_scenario_command(controller, &submit_trigger_order(delayed_02_id))
        .expect("put delayed_02 first so delayed_01 resolves on top");
    assert!(engine.state.pending_trigger_order.is_none());
    assert_eq!(
        engine.state.stack.last().unwrap().ability_program_owner(),
        Some(runtime_proof_child_owner()),
        "the actual staged child is the exact Arcane Denial owner"
    );
}

fn runtime_proof_child_key(
    engine: &GameEngine,
    controller: i32,
    affected_player: i32,
) -> tricerules_core::state::TriggerUseKey {
    let child = engine
        .state
        .stack
        .last()
        .expect("the ordered Arcane child is on top");
    assert_eq!(
        child.ability_program_owner(),
        Some(runtime_proof_child_owner())
    );
    assert_eq!(child.controller, controller);
    assert_eq!(child.trigger_context.affected_player, Some(affected_player));
    child
        .trigger_use_key
        .clone()
        .expect("the delayed Program has one exact trigger occurrence")
}

fn runtime_proof_assert_parked_key(
    engine: &GameEngine,
    key: &tricerules_core::state::TriggerUseKey,
) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Program is parked on the offered choice");
    let stack = pending
        .continuation
        .stack()
        .expect("the Program choice retains the delayed stack item");
    assert_eq!(stack.item.trigger_use_key.as_ref(), Some(key));
    assert_eq!(
        stack.item.ability_program_owner(),
        Some(runtime_proof_child_owner())
    );
}

fn runtime_proof_choose_nonland_match(
    engine: &mut GameEngine,
    drawer: i32,
    drawer_index: usize,
    batch: RuledEventBatch,
    expected_object_id: u32,
    child_key: &tricerules_core::state::TriggerUseKey,
    child_continues_after_draw: bool,
) -> RuledEventBatch {
    let kind = find_resolution_choice(&batch).expect("Abundance asks for land or nonland");
    assert_eq!(kind.deciding_player_id, drawer);
    assert_eq!(kind.choice_kind, ChoiceKind::ResolutionBranch as i32);
    let kind_batch = engine
        .apply_scenario_command(drawer, &runtime_proof_answer_branch(&kind, "Nonland"))
        .expect("choose Abundance's offered Nonland branch");
    if child_continues_after_draw {
        runtime_proof_assert_parked_key(engine, child_key);
    } else {
        assert!(
            engine.state.pending_resolution.is_none(),
            "the final Program Draw completes without retaining a finished child"
        );
        assert!(!engine
            .state
            .stack
            .iter()
            .any(|item| item.trigger_use_key.as_ref() == Some(child_key)));
    }
    assert!(
        engine.state.players[drawer_index]
            .hand
            .contains(&expected_object_id),
        "the matching top nonland moves to the drawer's hand automatically"
    );
    assert!(
        find_resolution_choice(&kind_batch)
            .map(|offer| offer.choice_kind != ChoiceKind::LibraryLook as i32)
            .unwrap_or(true),
        "an immediately matching top card needs no library-card selection"
    );
    kind_batch
}

fn synthetic_arcane_tail_choose_nonland(
    engine: &mut GameEngine,
    drawer: i32,
    batch: RuledEventBatch,
    expected_object_id: u32,
    child_key: &tricerules_core::state::TriggerUseKey,
    child_owner: &tricerules_cards::ProgramOwnerId,
    child_continues_after_draw: bool,
) -> RuledEventBatch {
    let drawer_index = engine
        .state
        .player_idx(drawer)
        .expect("drawer is a live player");
    let choice = find_resolution_choice(&batch).expect("Abundance asks for land or nonland");
    assert_eq!(choice.deciding_player_id, drawer);
    assert_eq!(choice.choice_kind, ChoiceKind::ResolutionBranch as i32);
    let resumed = engine
        .apply_scenario_command(drawer, &runtime_proof_answer_branch(&choice, "Nonland"))
        .expect("choose the offered Abundance Nonland replacement");
    if child_continues_after_draw {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the delayed Program remains parked on its next draw replacement");
        let stack = pending
            .continuation
            .stack()
            .expect("the replacement retains the delayed trigger stack item");
        assert_eq!(stack.item.trigger_use_key.as_ref(), Some(child_key));
        assert_eq!(
            stack.item.ability_program_owner(),
            Some(child_owner.clone())
        );
    } else {
        assert!(
            engine.state.pending_resolution.is_none(),
            "the final replacement resumes the GainLife tail and completes the child"
        );
        assert!(!engine
            .state
            .stack
            .iter()
            .any(|item| item.trigger_use_key.as_ref() == Some(child_key)));
    }
    assert!(engine.state.players[drawer_index]
        .hand
        .contains(&expected_object_id));
    assert!(
        find_resolution_choice(&resumed)
            .map(|offer| offer.choice_kind != ChoiceKind::LibraryLook as i32)
            .unwrap_or(true),
        "a matching top card moves to hand without a library-card picker"
    );
    resumed
}

fn synthetic_arcane_tail_accept_replace(
    engine: &mut GameEngine,
    drawer: i32,
    batch: RuledEventBatch,
    child_key: &tricerules_core::state::TriggerUseKey,
    child_owner: &tricerules_cards::ProgramOwnerId,
) -> RuledEventBatch {
    let choice = find_resolution_choice(&batch).expect("Abundance offers a draw replacement");
    assert_eq!(choice.deciding_player_id, drawer);
    let resumed = engine
        .apply_scenario_command(
            drawer,
            &runtime_proof_answer_branch(&choice, "Replace this draw"),
        )
        .expect("accept the offered Abundance replacement");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the delayed Program remains parked through replacement choice");
    let stack = pending
        .continuation
        .stack()
        .expect("the replacement retains the delayed trigger stack item");
    assert_eq!(stack.item.trigger_use_key.as_ref(), Some(child_key));
    assert_eq!(
        stack.item.ability_program_owner(),
        Some(child_owner.clone())
    );
    resumed
}

#[test]
fn twincast_copy_of_arcane_denial_counters_bolt_and_keeps_only_its_delayed_pair() {
    let [a, b, c] = PLAYER_IDS;
    let mut engine = runtime_proof_engine(84_201);
    inject_card_into_hand(&mut engine, 0, "arcane_denial");
    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    inject_card_into_hand(&mut engine, 2, "twincast");
    give_mana(
        &mut engine,
        a,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        b,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        c,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(a, &pass())
        .expect("A passes to B");
    let bolt_slot = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_scenario_command(b, &cast_spell(bolt_slot, target_player(a)))
        .expect("B casts Bolt at A");
    let bolt_id = engine.state.stack.last().unwrap().id;
    runtime_proof_pass_to(&mut engine, a);
    let denial_slot = hand_index_for_card(&engine, 0, "arcane_denial");
    engine
        .apply_scenario_command(a, &cast_spell(denial_slot, stack_target(bolt_id)))
        .expect("A casts Arcane Denial at B's Bolt");
    let arcane_id = engine.state.stack.last().unwrap().id;
    runtime_proof_pass_to(&mut engine, c);
    let twincast_slot = hand_index_for_card(&engine, 2, "twincast");
    engine
        .apply_scenario_command(c, &cast_spell(twincast_slot, stack_target(arcane_id)))
        .expect("C casts Twincast at A's Arcane Denial");

    let offered_batch = runtime_proof_pass_round_with_batch(&mut engine);
    let targets = find_resolution_choice(&offered_batch).expect("Twincast asks for copy targets");
    assert_eq!(targets.deciding_player_id, c);
    assert_ne!(targets.resolution_prompt_id, 0);
    assert!(targets.candidate_object_ids.contains(&bolt_id));
    let copied_arcane_batch = engine
        .apply_scenario_command(c, &runtime_proof_answer_objects(&targets, vec![bolt_id]))
        .expect("C selects B's actual Bolt stack item for the Arcane copy");
    let copy_push = copied_arcane_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(stack)) if stack.is_copy => Some(stack),
            _ => None,
        })
        .expect("Twincast resolution pushed an actual spell copy");
    assert_eq!(copy_push.card_id, "arcane_denial");
    assert!(copy_push.targets.iter().any(|target| {
        target.object_id == bolt_id && target.kind == TargetRefKind::Stack as i32
    }));
    let copy = engine.state.stack.last().expect("Arcane copy on the stack");
    let copy_id = copy.id;
    assert!(copy.is_copy);
    assert_eq!(copy.controller, c);
    assert_eq!(copy.cast_occurrence, None);
    assert!(!engine.state.objects.contains_key(&copy_id));

    runtime_proof_pass_round_with_batch(&mut engine);
    assert!(
        engine.state.objects.contains_key(&arcane_id),
        "the real Arcane card remains physical"
    );
    runtime_proof_assert_delayed_pair(&engine, copy_id, c, b);
    assert!(engine.state.stack.iter().any(|item| item.id == arcane_id));
    assert!(
        !engine.state.stack.iter().any(|item| item.id == bolt_id),
        "the copy countered Bolt"
    );

    runtime_proof_pass_round_with_batch(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "A's now-illegal original Arcane fizzled"
    );
    assert_eq!(
        engine.state.active_event_observers.len(),
        2,
        "the fizzle registers no second pair"
    );

    let b_hand_before = engine.state.players[1].hand.len();
    let c_hand_before = engine.state.players[2].hand.len();
    end_active_turn(&mut engine, a);
    assert_eq!(engine.state.active_player_id(), b);
    runtime_proof_order_child_first(&mut engine, c);
    let key = runtime_proof_child_key(&engine, c, b);
    let prompt_batch = runtime_proof_pass_round_with_batch(&mut engine);
    let draw_choice =
        find_resolution_choice(&prompt_batch).expect("B receives delayed_01's choice");
    assert_eq!(draw_choice.deciding_player_id, b);
    runtime_proof_assert_all_arcane_branch_presentations(&draw_choice);
    runtime_proof_assert_parked_key(&engine, &key);
    runtime_proof_reject_other_actors_atomically(
        &mut engine,
        &draw_choice,
        "The affected player draws 2 cards.",
        &[a, c],
    );
    engine
        .apply_scenario_command(
            b,
            &runtime_proof_answer_branch(&draw_choice, "The affected player draws 2 cards."),
        )
        .expect("B chooses two cards for the affected-player Program");
    while !engine.state.stack.is_empty() {
        runtime_proof_pass_round_with_batch(&mut engine);
    }
    assert_eq!(engine.state.players[1].hand.len(), b_hand_before + 2);
    assert_eq!(engine.state.players[2].hand.len(), c_hand_before + 1);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn arcane_denial_of_twincast_bolt_copy_captures_the_copy_controller() {
    let [a, b, c] = PLAYER_IDS;
    let mut engine = runtime_proof_engine(84_202);
    inject_card_into_hand(&mut engine, 0, "arcane_denial");
    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    inject_card_into_hand(&mut engine, 2, "twincast");
    give_mana(
        &mut engine,
        a,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        b,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        c,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(a, &pass())
        .expect("A passes to B");
    let bolt_slot = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_scenario_command(b, &cast_spell(bolt_slot, target_player(a)))
        .expect("B casts Bolt at A");
    let bolt_id = engine.state.stack.last().unwrap().id;
    let bolt = engine.state.stack.last().unwrap();
    assert_eq!(bolt.controller, b);
    assert_eq!(engine.state.objects[&bolt_id].owner, b);
    runtime_proof_pass_to(&mut engine, c);
    let twincast_slot = hand_index_for_card(&engine, 2, "twincast");
    engine
        .apply_scenario_command(c, &cast_spell(twincast_slot, stack_target(bolt_id)))
        .expect("C casts Twincast at B's real Bolt");

    let offered_batch = runtime_proof_pass_round_with_batch(&mut engine);
    let targets = find_resolution_choice(&offered_batch).expect("Twincast asks for copy targets");
    assert_eq!(targets.deciding_player_id, c);
    assert!(targets.candidate_player_ids.contains(&a));
    let copy_batch = engine
        .apply_scenario_command(c, &runtime_proof_answer_players(&targets, vec![a]))
        .expect("C chooses A as the copied Bolt's target");
    assert!(copy_batch.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::StackPushed(stack)) if stack.is_copy && stack.card_id == "lightning_bolt")
    ));
    let copy_bolt = engine.state.stack.last().expect("copied Bolt on the stack");
    let copy_id = copy_bolt.id;
    assert!(copy_bolt.is_copy);
    assert_eq!(copy_bolt.controller, c);
    assert!(!engine.state.objects.contains_key(&copy_id));
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .find(|item| item.id == bolt_id)
            .unwrap()
            .controller,
        b
    );

    let denial_slot = hand_index_for_card(&engine, 0, "arcane_denial");
    engine
        .apply_scenario_command(a, &cast_spell(denial_slot, stack_target(copy_id)))
        .expect("A targets C's Bolt copy, not B's physical Bolt");
    let arcane_id = engine.state.stack.last().unwrap().id;
    runtime_proof_pass_round_with_batch(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == copy_id));
    assert!(engine.state.stack.iter().any(|item| item.id == bolt_id));
    runtime_proof_assert_delayed_pair(&engine, arcane_id, a, c);
    runtime_proof_pass_round_with_batch(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "the remaining original Bolt resolves"
    );

    let c_hand_before = engine.state.players[2].hand.len();
    end_active_turn(&mut engine, a);
    assert_eq!(engine.state.active_player_id(), b);
    let a_hand_before = engine.state.players[0].hand.len();
    let a_cards_drawn_before = engine.state.turn_history.current.player(a).cards_drawn;
    let a_library_before = engine.state.players[0].library.len();
    runtime_proof_order_child_first(&mut engine, a);
    let key = runtime_proof_child_key(&engine, a, c);
    let prompt_batch = runtime_proof_pass_round_with_batch(&mut engine);
    let draw_choice =
        find_resolution_choice(&prompt_batch).expect("C receives delayed_01's choice");
    assert_eq!(draw_choice.deciding_player_id, c);
    runtime_proof_assert_arcane_branch_presentation(
        &draw_choice,
        "The affected player draws 2 cards.",
        "draw_two",
    );
    runtime_proof_assert_parked_key(&engine, &key);
    runtime_proof_reject_other_actors_atomically(
        &mut engine,
        &draw_choice,
        "The affected player draws 2 cards.",
        &[a, b],
    );
    engine
        .apply_scenario_command(
            c,
            &runtime_proof_answer_branch(&draw_choice, "The affected player draws 2 cards."),
        )
        .expect("C chooses for the copied Bolt's captured controller");
    while !engine.state.stack.is_empty() {
        runtime_proof_pass_round_with_batch(&mut engine);
    }
    assert_eq!(engine.state.players[2].hand.len(), c_hand_before + 2);
    assert_eq!(
        engine.state.players[0].hand.len(),
        a_hand_before + 1,
        "A's independent delayed_02 draw: cards_drawn {} -> {}, library {} -> {}, observers {}, stack {}",
        a_cards_drawn_before,
        engine.state.turn_history.current.player(a).cards_drawn,
        a_library_before,
        engine.state.players[0].library.len(),
        engine.state.active_event_observers.len(),
        engine.state.stack.len(),
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn synthetic_arcane_delayed_program_resumes_life_tail_after_two_abundance_replacements() {
    let [a, b, _c] = PLAYER_IDS;
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                SYNTHETIC_ARCANE_DELAYED_TAIL,
                include_str!("../../../tricerules-cards/data/abundance.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/lightning_bolt.ron"),
            ],
            &[],
        )
        .expect("the synthetic face uses the closed delayed-child profile with a Program tail"),
    ));
    let decks = Some(vec![vec!["island".into(); 20]; PLAYER_IDS.len()]);
    let mut engine = GameEngine::new(registry, 84_204, &PLAYER_IDS, 20, decks, true)
        .expect("new synthetic Arcane continuation game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 1, "abundance");
    let [first_nonland, second_nonland] =
        set_library_top(&mut engine, 1, &["lightning_bolt", "lightning_bolt"])
            .try_into()
            .expect("two top nonlands for independent Abundance replacements");
    inject_card_into_hand(&mut engine, 0, "test_arcane_delayed_tail");
    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    give_mana(
        &mut engine,
        a,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        b,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(a, &pass())
        .expect("A passes to B");
    let bolt_slot = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_scenario_command(b, &cast_spell(bolt_slot, target_player(a)))
        .expect("B casts Bolt at A");
    let bolt_id = engine.state.stack.last().unwrap().id;
    runtime_proof_pass_to(&mut engine, a);
    let delayed_spell_slot = hand_index_for_card(&engine, 0, "test_arcane_delayed_tail");
    engine
        .apply_scenario_command(a, &cast_spell(delayed_spell_slot, stack_target(bolt_id)))
        .expect("A counters B's spell with the synthetic closed-profile face");
    let delayed_spell_id = engine.state.stack.last().unwrap().id;
    runtime_proof_pass_round_with_batch(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "the synthetic counter resolves"
    );
    assert_eq!(engine.state.objects[&bolt_id].zone, Zone::Graveyard);
    assert_eq!(engine.state.active_event_observers.len(), 1);

    end_active_turn(&mut engine, a);
    assert_eq!(engine.state.active_player_id(), b);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert!(
        engine.state.stack.len() == 1,
        "the next-upkeep child is staged"
    );
    let child = engine.state.stack.last().unwrap();
    let child_owner =
        tricerules_cards::ProgramOwnerId::ability(tricerules_cards::AbilityDefinitionId {
            card_id: "test_arcane_delayed_tail".into(),
            face_id: tricerules_cards::CardFaceId::new("test_arcane_delayed_tail").unwrap(),
            ability_path: vec![tricerules_cards::AbilityId::new("delayed_01").unwrap()],
        });
    assert_eq!(child.ability_program_owner(), Some(child_owner.clone()));
    assert_eq!(child.controller, a);
    assert_eq!(child.trigger_context.affected_player, Some(b));
    let child_key = child
        .trigger_use_key
        .clone()
        .expect("synthetic delayed child has one exact trigger occurrence");
    let a_life_before = engine.state.players[0].life;
    let b_life_before = engine.state.players[1].life;
    let b_library_before = engine.state.players[1].library.len();
    let b_draws_before = engine.state.turn_history.current.player(b).cards_drawn;
    let choice_batch = runtime_proof_pass_round_with_batch(&mut engine);
    let count_choice = find_resolution_choice(&choice_batch)
        .expect("the affected player receives the synthetic Draw-count choice");
    assert_eq!(count_choice.deciding_player_id, b);
    assert_eq!(count_choice.resolution_branches.len(), 3);
    let draw_two_label = count_choice.resolution_branches[2].label.clone();
    let draw_two = engine
        .apply_scenario_command(
            b,
            &runtime_proof_answer_branch(&count_choice, &draw_two_label),
        )
        .expect("B selects the draw-two branch from the actual offer");
    assert_eq!(engine.state.players[0].life, a_life_before);
    assert_eq!(engine.state.players[1].life, b_life_before);

    let first_kind =
        synthetic_arcane_tail_accept_replace(&mut engine, b, draw_two, &child_key, &child_owner);
    let after_first = synthetic_arcane_tail_choose_nonland(
        &mut engine,
        b,
        first_kind,
        first_nonland,
        &child_key,
        &child_owner,
        true,
    );
    assert_eq!(engine.state.players[0].life, a_life_before);
    assert_eq!(engine.state.players[1].life, b_life_before);
    let second_replacement = find_resolution_choice(&after_first)
        .expect("the second Draw has its own Abundance replacement prompt");
    assert_eq!(second_replacement.deciding_player_id, b);

    let second_kind =
        synthetic_arcane_tail_accept_replace(&mut engine, b, after_first, &child_key, &child_owner);
    synthetic_arcane_tail_choose_nonland(
        &mut engine,
        b,
        second_kind,
        second_nonland,
        &child_key,
        &child_owner,
        false,
    );
    assert_eq!(engine.state.players[0].life, a_life_before + 1);
    assert_eq!(engine.state.players[1].life, b_life_before);
    assert_eq!(engine.state.players[1].library.len(), b_library_before - 2);
    assert_eq!(
        engine.state.turn_history.current.player(b).cards_drawn,
        b_draws_before
    );
    assert!(engine.state.players[1].hand.contains(&first_nonland));
    assert!(engine.state.players[1].hand.contains(&second_nonland));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.active_event_observers.is_empty());
    assert_eq!(
        engine.state.objects[&delayed_spell_id].zone,
        Zone::Graveyard
    );
}

#[test]
fn arcane_denial_program_draw_two_resumes_two_abundance_replacements_once() {
    let [a, b, c] = PLAYER_IDS;
    let mut engine = runtime_proof_engine(84_203);
    inject_card_into_hand(&mut engine, 0, "arcane_denial");
    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    inject_permanent_on_battlefield(&mut engine, 1, "abundance");
    let b_nonlands = set_library_top(&mut engine, 1, &["grizzly_bears", "storm_crow"]);
    let a_top = set_library_top(&mut engine, 0, &["forest"]);
    give_mana(
        &mut engine,
        a,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        b,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(a, &pass())
        .expect("A passes to B");
    let bolt_slot = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_scenario_command(b, &cast_spell(bolt_slot, target_player(a)))
        .expect("B casts Bolt; it has no draw effect to contaminate Abundance");
    let bolt_id = engine.state.stack.last().unwrap().id;
    runtime_proof_pass_to(&mut engine, a);
    let denial_slot = hand_index_for_card(&engine, 0, "arcane_denial");
    engine
        .apply_scenario_command(a, &cast_spell(denial_slot, stack_target(bolt_id)))
        .expect("A casts Arcane Denial at B's Bolt");
    let arcane_id = engine.state.stack.last().unwrap().id;
    runtime_proof_pass_round_with_batch(&mut engine);
    runtime_proof_assert_delayed_pair(&engine, arcane_id, a, b);

    end_active_turn(&mut engine, a);
    assert_eq!(engine.state.active_player_id(), b);
    runtime_proof_order_child_first(&mut engine, a);
    let key = runtime_proof_child_key(&engine, a, b);
    let prompt_batch = runtime_proof_pass_round_with_batch(&mut engine);
    let draw_choice =
        find_resolution_choice(&prompt_batch).expect("B receives delayed_01's choice");
    assert_eq!(draw_choice.deciding_player_id, b);
    runtime_proof_assert_arcane_branch_presentation(
        &draw_choice,
        "The affected player draws 2 cards.",
        "draw_two",
    );
    runtime_proof_assert_parked_key(&engine, &key);
    runtime_proof_reject_other_actors_atomically(
        &mut engine,
        &draw_choice,
        "The affected player draws 2 cards.",
        &[a, c],
    );
    let b_hand_before = engine.state.players[1].hand.len();
    let a_hand_before = engine.state.players[0].hand.len();
    let b_library_before = engine.state.players[1].library.len();
    let a_library_before = engine.state.players[0].library.len();
    let b_actual_draws_before = engine.state.turn_history.current.player(b).cards_drawn;
    let a_actual_draws_before = engine.state.turn_history.current.player(a).cards_drawn;

    let first_abundance = engine
        .apply_scenario_command(
            b,
            &runtime_proof_answer_branch(&draw_choice, "The affected player draws 2 cards."),
        )
        .expect("B chooses the Program's offered two-card branch");
    runtime_proof_assert_parked_key(&engine, &key);
    let first_replace = find_resolution_choice(&first_abundance)
        .expect("the first Draw independently offers Abundance");
    assert_eq!(first_replace.deciding_player_id, b);
    assert_eq!(
        first_replace.choice_kind,
        ChoiceKind::ResolutionBranch as i32
    );
    let replace_command = runtime_proof_answer_branch(&first_replace, "Replace this draw");
    let before_wrong_actor = engine
        .diagnostic_snapshot()
        .expect("snapshot before wrong chooser");
    assert!(engine.apply_scenario_command(a, &replace_command).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after rejected wrong chooser"),
        before_wrong_actor,
        "only B, the affected drawer, can choose B's Abundance replacement"
    );
    let first_kind = engine
        .apply_scenario_command(b, &replace_command)
        .expect("B accepts the first Abundance replacement");
    runtime_proof_assert_parked_key(&engine, &key);
    let second_abundance = runtime_proof_choose_nonland_match(
        &mut engine,
        b,
        1,
        first_kind,
        b_nonlands[0],
        &key,
        true,
    );
    let second_replace = find_resolution_choice(&second_abundance)
        .expect("the Program's second Draw independently offers Abundance");
    assert_eq!(second_replace.deciding_player_id, b);
    assert_eq!(
        second_replace.choice_kind,
        ChoiceKind::ResolutionBranch as i32
    );
    let second_kind = engine
        .apply_scenario_command(
            b,
            &runtime_proof_answer_branch(&second_replace, "Replace this draw"),
        )
        .expect("B accepts the second Abundance replacement");
    let finished_program = runtime_proof_choose_nonland_match(
        &mut engine,
        b,
        1,
        second_kind,
        b_nonlands[1],
        &key,
        false,
    );
    assert!(find_resolution_choice(&finished_program).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.stack.len(),
        1,
        "only delayed_02 remains to resolve"
    );
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.trigger_use_key.as_ref() == Some(&key)));
    assert_eq!(engine.state.players[1].hand.len(), b_hand_before + 2);
    assert!(engine.state.players[1].hand.contains(&b_nonlands[0]));
    assert!(engine.state.players[1].hand.contains(&b_nonlands[1]));
    assert_eq!(engine.state.players[1].library.len(), b_library_before - 2);
    assert_eq!(
        engine.state.turn_history.current.player(b).cards_drawn,
        b_actual_draws_before,
        "Abundance replaced both attempts; B had no actual CardDrawn events"
    );

    while !engine.state.stack.is_empty() {
        runtime_proof_pass_round_with_batch(&mut engine);
    }
    assert_eq!(engine.state.players[0].hand.len(), a_hand_before + 1);
    assert!(engine.state.players[0].hand.contains(&a_top[0]));
    assert_eq!(engine.state.players[0].library.len(), a_library_before - 1);
    assert_eq!(
        engine.state.turn_history.current.player(a).cards_drawn,
        a_actual_draws_before + 1,
        "Arcane Denial's delayed_02 draws once for its caster, outside B's Abundance"
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
