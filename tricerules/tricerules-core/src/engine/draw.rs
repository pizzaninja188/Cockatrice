//! CR 121/614/616: one resumable transaction for actual draw instructions.
use super::characteristics::printed_static_source_is_available;
use super::events::{ev_log, ev_priority_changed, finish_with_events};
use super::replacement::PendingReplacementEvent;
use super::triggers::ability_definition_from;
use super::*;
use tricerules_cards::primitives::{DrawReplacementCondition, LibraryDrawReplacement};
mod library_actions;

#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
struct DrawReplacementIdentity {
    source: ObjectId,
    generation: u64,
    definition: AbilityDefinitionId,
}

#[cfg(test)]
mod library_replacement_tests {
    use super::*;

    fn fixture(kind: &str, size: usize) -> GameEngine {
        let source = format!(
            r#"(id: "replacement_fixture", name: "Replacement fixture", face_id: "replacement_fixture", types: ["Creature"], power: 2, toughness: 3, static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ReplaceControllerDrawWithLibraryChoice(kind: {kind}))])"#
        );
        let registry = CardRegistry::from_chunks_and_tokens(&[
            r#"(id: "forest", name: "Forest", face_id: "forest", types: ["Basic", "Land", "Forest"])"#,
            r#"(id: "nonland_fixture", name: "Nonland fixture", face_id: "nonland_fixture", types: ["Creature"], power: 1, toughness: 1)"#,
            r#"(id: "double_fixture", name: "Double fixture", face_id: "double_fixture", types: ["Enchantment"], static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: DoubleControllerDraws(condition: Always))])"#,
            &source,
        ], &[]).expect("admit the real draw replacement operation");
        let mut engine = GameEngine::new(
            12106,
            &[0, 1, 2],
            20,
            Some(vec![vec!["forest".into(); 24]; 3]),
            true,
        )
        .unwrap();
        engine.registry = Box::leak(Box::new(registry));
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Draw;
        engine.state.draw_step_progress = Some((0, 0, 0));
        let source = engine.state.players[0].library[0];
        resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        engine.state.objects.get_mut(&source).unwrap().card_id = "replacement_fixture".into();
        let excess: Vec<_> = engine.state.players[0]
            .library
            .iter()
            .skip(size)
            .copied()
            .collect();
        for oid in excess {
            resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Exile,
                None,
            )
            .unwrap();
        }
        engine
    }

    fn start(engine: &mut GameEngine) -> (DrawProgress, Vec<rv1::RuledEvent>) {
        let mut events = Vec::new();
        let progress = engine
            .start_draw_transaction(
                vec![(0, 1)],
                DrawCompletion::FinishDrawStep {
                    active_player: 0,
                    occurrence: 0,
                },
                "fixture",
                &mut events,
            )
            .unwrap();
        (progress, events)
    }

    #[test]
    fn authoring_tomorrow_replacement_handles_short_and_empty_libraries_without_drawing() {
        for size in 0..=3 {
            let mut engine = fixture("LookAtTopThree", size);
            let hand = engine.state.players[0].hand.len();
            let looked: Vec<_> = engine.state.players[0].library.iter().copied().collect();
            let (progress, events) = start(&mut engine);
            if size == 0 {
                let DrawProgress::Complete(done) = progress else {
                    panic!("empty replacement completes")
                };
                assert!(done.receipts.is_empty());
                assert!(events.iter().all(|event| !matches!(
                    event.ev,
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
                )));
            } else {
                assert!(matches!(progress, DrawProgress::Parked));
                let pending = engine.state.pending_resolution.as_ref().unwrap();
                assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
                assert_eq!(pending.presentation.candidates, looked);
                assert!(pending.continuation.stack().is_none());
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![looked[0]],
                            ..Default::default()
                        },
                    )
                    .unwrap();
                if size == 3 {
                    engine
                        .submit_resolution_choice(
                            0,
                            &rv1::SubmitResolutionChoice {
                                chosen_object_ids: vec![looked[2], looked[1]],
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
                assert_eq!(engine.state.players[0].hand.len(), hand + 1);
                assert_eq!(
                    engine.state.players[0]
                        .library
                        .iter()
                        .copied()
                        .collect::<Vec<_>>(),
                    if size == 3 {
                        vec![looked[2], looked[1]]
                    } else {
                        looked[1..].to_vec()
                    }
                );
            }
            assert!(!engine.state.players[0].pending_library_loss);
            assert_eq!(
                engine.resolve_amount(
                    &Amount::Count(CountExpression::CardsDrawnThisTurn {
                        players: RelativePlayerSet::Controller,
                    }),
                    AmountContext::from_condition(ConditionContext {
                        controller: 0,
                        source_object_id: 0,
                        source_zone_change: 0,
                        resolving_spell_id: None,
                        stack_item: None,
                        previous_effect_result: None,
                    })
                ),
                0
            );
        }
    }

    fn branch(engine: &mut GameEngine, index: u32) -> RuledEventBatch {
        engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            selected_branch_index: index,
                            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap()
    }

    #[test]
    fn authoring_draw_branch_commands_reject_atomically_and_repeat_deterministically() {
        let run = || {
            let mut engine = fixture("RevealUntilLandOrNonland", 4);
            let cards: Vec<_> = engine.state.players[0].library.iter().copied().collect();
            for oid in &cards[..2] {
                engine.state.objects.get_mut(oid).unwrap().card_id = "nonland_fixture".into();
            }
            let (_, initial) = start(&mut engine);
            let mut batches = vec![format!("{initial:?}")];
            for stage in 0..2 {
                let before = serde_json::to_value(&engine.state).unwrap();
                for answer in [
                    rv1::SubmitResolutionChoice::default(),
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::Decline as i32,
                        ..Default::default()
                    },
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                        chosen_object_ids: vec![cards[0]],
                        ..Default::default()
                    },
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                        selected_branch_index: 2,
                        ..Default::default()
                    },
                ] {
                    assert!(engine
                        .apply_command(
                            0,
                            &rv1::RuledCommand {
                                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
                            }
                        )
                        .is_err());
                    assert_eq!(
                        serde_json::to_value(&engine.state).unwrap(),
                        before,
                        "stage {stage}: rejection changed continuation or command index"
                    );
                }
                batches.push(format!("{:?}", branch(&mut engine, 0)));
            }
            assert!(engine.state.pending_resolution.is_some());
            let done = engine
                .apply_command(
                    0,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice {
                                chosen_object_ids: vec![cards[1], cards[0]],
                                ..Default::default()
                            },
                        )),
                    },
                )
                .unwrap();
            batches.push(format!("{done:?}"));
            assert!(engine.state.pending_resolution.is_none());
            (batches, serde_json::to_value(&engine.state).unwrap())
        };
        assert_eq!(
            run(),
            run(),
            "same seed and stock commands must reproduce every event batch and final state"
        );
    }

    #[test]
    fn authoring_abundance_reveals_matching_prefix_and_keeps_bottom_order_private() {
        let mut engine = fixture("RevealUntilLandOrNonland", 4);
        let cards: Vec<_> = engine.state.players[0].library.iter().copied().collect();
        for oid in &cards[..2] {
            engine.state.objects.get_mut(oid).unwrap().card_id = "nonland_fixture".into();
        }
        let hand = engine.state.players[0].hand.len();
        assert!(matches!(start(&mut engine).0, DrawProgress::Parked));
        branch(&mut engine, 0); // replace
        let batch = branch(&mut engine, 0); // land
        let reveal = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) => Some(reveal.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            reveal
                .cards
                .iter()
                .map(|card| card.object_id)
                .collect::<Vec<_>>(),
            cards[..3]
        );
        assert!(!reveal.reveal_id.is_empty());
        assert_eq!(engine.state.players[0].hand.len(), hand + 1);
        assert!(engine.state.players[0].hand.contains(&cards[2]));
        let event = engine.draw_replacement_choice_event().unwrap();
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = event.ev else {
            panic!("ordering choice")
        };
        assert!(choice.public_reveal.is_none());
        assert_eq!(choice.candidate_object_ids, cards[..2]);
        assert!(choice.ordered);
        assert_eq!(engine.draw_action_reveal(), Some(reveal.clone()));
        assert_eq!(
            engine.draw_replacement_choice_event(),
            Some(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            })
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![cards[1], cards[0]],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            engine.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![cards[3], cards[1], cards[0]]
        );
        assert!(engine.draw_action_reveal().is_none());
        assert_eq!(engine.state.draw_step_progress, Some((0, 0, 0)));
    }

    #[test]
    fn authoring_abundance_decline_no_match_and_empty_do_not_restart_the_replacement() {
        for size in [0, 1, 3] {
            let mut engine = fixture("RevealUntilLandOrNonland", size);
            let hand = engine.state.players[0].hand.len();
            let cards: Vec<_> = engine.state.players[0].library.iter().copied().collect();
            start(&mut engine);
            branch(&mut engine, 0);
            branch(&mut engine, 1); // no nonlands: consume the draw, order the entire revealed cohort
            if size > 1 {
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: cards.iter().rev().copied().collect(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.state.players[0].hand.len(), hand);
            assert!(!engine.state.players[0].pending_library_loss);
            assert_eq!(engine.state.draw_step_progress, Some((0, 0, 0)));
        }
        let mut engine = fixture("RevealUntilLandOrNonland", 2);
        let hand = engine.state.players[0].hand.len();
        start(&mut engine);
        branch(&mut engine, 1);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[0].hand.len(), hand + 1);
        assert_eq!(engine.state.draw_step_progress, Some((0, 0, 1)));
    }

    #[test]
    fn authoring_draw_action_rejects_bad_answers_atomically_and_survives_source_departure() {
        let mut engine = fixture("LookAtTopThree", 3);
        start(&mut engine);
        let cards = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .clone();
        let answer = rv1::SubmitResolutionChoice {
            chosen_object_ids: vec![cards[0]],
            ..Default::default()
        };
        let before = format!("{:?}", engine.state.pending_resolution);
        let work_before = format!("{:?}", engine.state.pending_replacement_event);
        assert!(engine.submit_resolution_choice(1, &answer).is_err());
        for bad in [
            rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![],
                ..Default::default()
            },
            rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![cards[0], cards[0]],
                ..Default::default()
            },
            rv1::SubmitResolutionChoice {
                chosen_player_ids: vec![1],
                ..answer.clone()
            },
        ] {
            assert!(engine.submit_resolution_choice(0, &bad).is_err());
            assert_eq!(format!("{:?}", engine.state.pending_resolution), before);
            assert_eq!(
                format!("{:?}", engine.state.pending_replacement_event),
                work_before
            );
        }
        *engine
            .state
            .zone_change_generation
            .entry(cards[0])
            .or_default() += 1;
        assert!(engine.submit_resolution_choice(0, &answer).is_err());
        *engine
            .state
            .zone_change_generation
            .get_mut(&cards[0])
            .unwrap() -= 1;
        let source = engine.state.players[0].battlefield[0];
        engine.state.players[0]
            .battlefield
            .retain(|oid| *oid != source);
        engine.state.players[1].battlefield.push(source);
        engine.state.objects.get_mut(&source).unwrap().owner = 1;
        engine.concede_batch(1).unwrap();
        assert!(!engine.state.objects.contains_key(&source));
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            cards
        );
        engine.submit_resolution_choice(0, &answer).unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![cards[2], cards[1]],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn authoring_drawer_departure_retires_only_their_action_and_continues_other_requests() {
        let mut engine = fixture("LookAtTopThree", 3);
        let hand = engine.state.players[2].hand.len();
        engine
            .start_draw_transaction(
                vec![(0, 1), (2, 1)],
                DrawCompletion::FinishDrawStep {
                    active_player: 0,
                    occurrence: 0,
                },
                "fixture",
                &mut Vec::new(),
            )
            .unwrap();
        engine.concede_batch(0).unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
        assert_eq!(engine.state.players[2].hand.len(), hand + 1);
    }

    #[test]
    fn authoring_doubling_and_library_replacement_finish_each_child_before_the_next() {
        for double_first in [false, true] {
            let mut engine = fixture("LookAtTopThree", 7);
            let double = engine.state.players[0].library[6];
            resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                double,
                Zone::Battlefield,
                None,
            )
            .unwrap();
            engine.state.objects.get_mut(&double).unwrap().card_id = "double_fixture".into();
            let hand = engine.state.players[0].hand.len();
            start(&mut engine);
            let Some(PendingReplacementEvent::Draw(work)) = &engine.state.pending_replacement_event
            else {
                panic!("replacement order")
            };
            let selected = work
                .applications
                .iter()
                .find(|application| {
                    matches!(application.action, DrawReplacementAction::Double) == double_first
                })
                .unwrap()
                .handle;
            engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![selected],
                        ..Default::default()
                    },
                )
                .unwrap();
            for _ in 0..if double_first { 2 } else { 1 } {
                let candidates = engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .candidates
                    .clone();
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![candidates[0]],
                            ..Default::default()
                        },
                    )
                    .unwrap();
                let bottom = engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .candidates
                    .clone();
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: bottom.into_iter().rev().collect(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(
                engine.state.players[0].hand.len(),
                hand + if double_first { 2 } else { 1 }
            );
            assert_eq!(engine.state.draw_step_progress, Some((0, 0, 0)));
        }
    }
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawApplication {
    handle: u32,
    identity: DrawReplacementIdentity,
    action: DrawReplacementAction,
    presentation: crate::state::ReplacementSourcePresentation,
}

#[derive(serde::Serialize, Debug, Clone, Copy)]
enum DrawReplacementAction {
    Double,
    WinInstead,
    Library(LibraryDrawReplacement),
}

enum AppliedDrawProgress {
    Continue,
    Parked,
    GameEnded,
}

#[derive(serde::Serialize, Debug, Clone)]
enum DrawLibraryStage {
    Optional,
    ChooseKind,
    ChooseToHand(Vec<(ObjectId, u64)>),
    OrderBottom(Vec<(ObjectId, u64)>),
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawLibraryAction {
    source: ObjectId,
    source_label: String,
    stage: DrawLibraryStage,
    reveal: Option<rv1::CardsRevealed>,
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawNode {
    applied: Vec<DrawReplacementIdentity>,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) enum DrawCompletion {
    ResumeEffects {
        stack: ParkedStackResolution,
        result: EffectResult,
    },
    BeginDiscard {
        stack: ParkedStackResolution,
        player: PlayerId,
        count: u32,
        result: EffectResult,
    },
    FinishDrawStep {
        active_player: PlayerId,
        occurrence: u64,
    },
    BrainstormPutBack {
        stack: ParkedStackResolution,
        step: u32,
        scratch: Vec<ObjectId>,
    },
}

impl DrawCompletion {
    fn stack(&self) -> Option<&ParkedStackResolution> {
        match self {
            Self::ResumeEffects { stack, .. }
            | Self::BeginDiscard { stack, .. }
            | Self::BrainstormPutBack { stack, .. } => Some(stack),
            Self::FinishDrawStep { .. } => None,
        }
    }
    fn transfer_stack(&mut self, parent: Option<ParkedStackResolution>) {
        if let Some(parent) = parent {
            match self {
                Self::ResumeEffects { stack, .. }
                | Self::BeginDiscard { stack, .. }
                | Self::BrainstormPutBack { stack, .. } => *stack = parent,
                Self::FinishDrawStep { .. } => {}
            }
        }
    }
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawRequest {
    player: PlayerId,
    remaining: u32,
    drawn: u32,
    failed: bool,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct PendingDrawTransaction {
    requests: VecDeque<DrawRequest>,
    nodes: Vec<DrawNode>,
    applications: Vec<DrawApplication>,
    completion: DrawCompletion,
    label: String,
    receipts: Vec<TriggerObjectRef>,
    action: Option<DrawLibraryAction>,
}

pub(super) struct CompletedDraw {
    pub completion: DrawCompletion,
    pub receipts: Vec<TriggerObjectRef>,
}
pub(super) enum DrawProgress {
    Complete(Box<CompletedDraw>),
    Parked,
    GameEnded,
}

pub(super) fn controller_library_empty(state: &GameState, controller: PlayerId) -> bool {
    state.player_idx(controller).is_some_and(|index| {
        !state.players[index].has_lost && state.players[index].library.is_empty()
    })
}

impl GameEngine {
    fn draw_candidates(
        &self,
        drawer: PlayerId,
        node: &DrawNode,
    ) -> Vec<(DrawReplacementIdentity, DrawReplacementAction)> {
        let first_own_step = self.state.turn_step == TurnStep::Draw
            && self.state.active_player_id() == drawer
            && self
                .state
                .draw_step_progress
                .is_some_and(|(player, _, count)| player == drawer && count == 0);
        let mut sources = self.state.objects.keys().copied().collect::<Vec<_>>();
        sources.sort_unstable();
        let mut result = Vec::new();
        for source in sources {
            if !printed_static_source_is_available(&self.state, self.registry, source)
                || self
                    .characteristics(source)
                    .is_none_or(|value| value.controller != drawer)
            {
                continue;
            }
            let Some(face) = self.effective_face(source) else {
                continue;
            };
            for ability in &face.static_abilities {
                let action = match ability.definition {
                    StaticAbilityDef::DoubleControllerDraws { condition } => {
                        if condition
                            == DrawReplacementCondition::ExceptFirstSuccessfulDrawInOwnDrawStep
                            && first_own_step
                        {
                            continue;
                        }
                        DrawReplacementAction::Double
                    }
                    StaticAbilityDef::WinControllerInsteadOfEmptyLibraryDraw
                        if controller_library_empty(&self.state, drawer) =>
                    {
                        DrawReplacementAction::WinInstead
                    }
                    StaticAbilityDef::ReplaceControllerDrawWithLibraryChoice { kind } => {
                        DrawReplacementAction::Library(kind)
                    }
                    _ => continue,
                };
                let identity = DrawReplacementIdentity {
                    source,
                    generation: self
                        .state
                        .zone_change_generation
                        .get(&source)
                        .copied()
                        .unwrap_or(0),
                    definition: ability_definition_from(
                        &self.state,
                        self.registry,
                        source,
                        self.state.objects[&source].face_up_index,
                        vec![ability.ability_id.clone()],
                    ),
                };
                if !node.applied.contains(&identity) {
                    result.push((identity, action));
                }
            }
        }
        result
    }

    pub(super) fn start_draw_transaction(
        &mut self,
        requests: Vec<(PlayerId, u32)>,
        completion: DrawCompletion,
        label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<DrawProgress, EngineError> {
        self.advance_draw_transaction(
            PendingDrawTransaction {
                requests: requests
                    .into_iter()
                    .map(|(player, remaining)| DrawRequest {
                        player,
                        remaining,
                        drawn: 0,
                        failed: false,
                    })
                    .collect(),
                nodes: Vec::new(),
                applications: Vec::new(),
                completion,
                label: label.to_string(),
                receipts: Vec::new(),
                action: None,
            },
            events,
        )
    }

    fn advance_draw_transaction(
        &mut self,
        mut work: PendingDrawTransaction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<DrawProgress, EngineError> {
        if self.state.is_terminal() {
            return Ok(DrawProgress::GameEnded);
        }
        while let Some(request) = work.requests.front_mut() {
            let Some(index) = self
                .state
                .player_idx(request.player)
                .filter(|index| !self.state.players[*index].has_lost)
            else {
                work.requests.pop_front();
                work.nodes.clear();
                work.action = None;
                continue;
            };
            if work.action.is_some() {
                return Ok(self.park_draw_library_action(work, events));
            }
            if work.nodes.is_empty() {
                if request.remaining == 0 {
                    events.push(ev_log(format!(
                        "P{} draws {} {} ({}).",
                        request.player,
                        request.drawn,
                        if request.drawn == 1 { "card" } else { "cards" },
                        work.label
                    )));
                    if request.failed {
                        events.push(ev_log(format!("P{} attempted to draw more cards than remained in the library; loss pending until the next state-based-action check (CR 121.4/704.5b; CR 704.4).", request.player)));
                    }
                    work.requests.pop_front();
                    continue;
                }
                request.remaining -= 1;
                work.nodes.push(DrawNode {
                    applied: Vec::new(),
                });
            }
            let node = work.nodes.last().expect("current draw node");
            let candidates = self.draw_candidates(request.player, node);
            if candidates.len() > 1 {
                work.applications.clear();
                for (identity, action) in candidates {
                    let handle = self.state.next_replacement_application_id;
                    self.state.next_replacement_application_id = handle.checked_add(1).ok_or(
                        EngineError::Illegal("replacement application ids exhausted"),
                    )?;
                    work.applications.push(DrawApplication {
                        handle,
                        presentation: self.replacement_source_presentation(identity.source),
                        identity,
                        action,
                    });
                }
                let player = request.player;
                let stack = work.completion.stack().cloned();
                let source_object_id = stack.as_ref().map_or(0, |stack| stack.item.id);
                let candidates = work.applications.iter().map(|value| value.handle).collect();
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::Draw(Box::new(work)));
                self.state.pending_resolution = Some(PendingResolution {
                    deciding_player: player,
                    presentation: PendingResolutionPresentation {
                        source_object_id,
                        candidates,
                        min: 1,
                        max: 1,
                        ordered: false,
                        unique_names: false,
                        prompt: "Choose which draw replacement applies next.".to_string(),
                        choice_kind: rv1::ChoiceKind::ReplacementEffect,
                    },
                    continuation: ResolutionContinuation::DrawReplacement { stack },
                });
                events.push(
                    self.draw_replacement_choice_event()
                        .expect("parked draw choice"),
                );
                return Ok(DrawProgress::Parked);
            }
            if let Some((identity, action)) = candidates.into_iter().next() {
                match self.apply_draw_replacement(&mut work, identity, action) {
                    AppliedDrawProgress::GameEnded => return Ok(DrawProgress::GameEnded),
                    AppliedDrawProgress::Parked => {
                        return Ok(self.park_draw_library_action(work, events))
                    }
                    AppliedDrawProgress::Continue => {}
                }
                continue;
            }
            work.nodes.pop().expect("current draw node");
            if let Some(oid) = self.state.players[index].library.front().copied() {
                resolution::draw_card(&mut self.state, self.registry, request.player)?;
                if self.state.turn_step == TurnStep::Draw
                    && self.state.active_player_id() == request.player
                {
                    if let Some((player, _, count)) = self.state.draw_step_progress.as_mut() {
                        if *player == request.player {
                            *count = count.saturating_add(1);
                        }
                    }
                }
                work.receipts.push(TriggerObjectRef {
                    object_id: oid,
                    zone_change_generation: self.state.zone_change_generation[&oid],
                    controller_at_event: request.player,
                });
                request.drawn += 1;
                self.fire_card_drawn(request.player, events);
            } else {
                self.state.players[index].pending_library_loss = true;
                request.failed = true;
            }
        }
        Ok(DrawProgress::Complete(Box::new(CompletedDraw {
            completion: work.completion,
            receipts: work.receipts,
        })))
    }

    fn apply_draw_replacement(
        &mut self,
        work: &mut PendingDrawTransaction,
        identity: DrawReplacementIdentity,
        action: DrawReplacementAction,
    ) -> AppliedDrawProgress {
        let mut node = work.nodes.pop().expect("current draw node");
        let source = identity.source;
        node.applied.push(identity);
        match action {
            DrawReplacementAction::Double => {
                // Children inherit ancestors; a subsequently modified sibling never changes them.
                work.nodes.push(node.clone());
                work.nodes.push(node);
                AppliedDrawProgress::Continue
            }
            DrawReplacementAction::WinInstead => {
                // CR 614.6: consume the draw before attempting the replacement's win. Even a
                // future prohibited win cannot restore this node or create a failed draw.
                self.state
                    .outcome
                    .get_or_insert(crate::state::GameOutcome::Winner(
                        work.requests.front().expect("drawer").player,
                    ));
                AppliedDrawProgress::GameEnded
            }
            DrawReplacementAction::Library(kind) => {
                let player = work.requests.front().expect("drawer").player;
                let stage = match kind {
                    LibraryDrawReplacement::RevealUntilLandOrNonland => {
                        // Declining leaves the modified node with this identity already applied.
                        work.nodes.push(node);
                        DrawLibraryStage::Optional
                    }
                    LibraryDrawReplacement::LookAtTopThree => {
                        let index = self.state.player_idx(player).expect("drawer");
                        let cards = self.state.players[index]
                            .library
                            .iter()
                            .take(3)
                            .copied()
                            .collect::<Vec<_>>();
                        if cards.is_empty() {
                            return AppliedDrawProgress::Continue;
                        }
                        DrawLibraryStage::ChooseToHand(super::library_choices::capture(
                            self, &cards,
                        ))
                    }
                };
                work.action = Some(DrawLibraryAction {
                    source,
                    source_label: super::events::object_display_name(
                        &self.state,
                        self.registry,
                        source,
                    ),
                    stage,
                    reveal: None,
                });
                AppliedDrawProgress::Parked
            }
        }
    }

    pub(super) fn draw_replacement_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let Some(PendingReplacementEvent::Draw(work)) = &self.state.pending_replacement_event
        else {
            return None;
        };
        if work.action.is_some() {
            return Some(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    self.draw_library_choice(work),
                )),
            });
        }
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                    candidate_object_ids: pending.presentation.candidates.clone(),
                    min: 1,
                    max: 1,
                    candidate_selectable: vec![true; work.applications.len()],
                    replacement_options: work
                        .applications
                        .iter()
                        .map(|value| {
                            value.presentation.option(
                                value.handle,
                                match value.action {
                                    DrawReplacementAction::Double => "Draw two cards instead.",
                                    DrawReplacementAction::WinInstead => "Win the game instead.",
                                    DrawReplacementAction::Library(LibraryDrawReplacement::LookAtTopThree) => "Look at three cards and put one into your hand instead.",
                                    DrawReplacementAction::Library(LibraryDrawReplacement::RevealUntilLandOrNonland) => "Choose whether to replace this draw with a land/nonland reveal.",
                                }
                                .to_string(),
                            )
                        })
                        .collect(),
                    ..Default::default()
                },
            )),
        })
    }

    pub(super) fn finish_draw_replacement_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let Some(PendingReplacementEvent::Draw(work)) = &self.state.pending_replacement_event
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("draw replacement event missing"));
        };
        if work.action.is_some() {
            return self.finish_draw_library_choice(pending, answer, decision);
        }
        let selected = (decision == rv1::ResolutionChoiceDecision::Unspecified
            && answer.chosen_object_ids.len() == 1
            && answer.chosen_player_ids.is_empty()
            && answer.selected_branch_index == 0
            && answer.cast_spell.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.spell_cast_announcement.is_none())
        .then(|| {
            work.applications
                .iter()
                .find(|value| value.handle == answer.chosen_object_ids[0])
        })
        .flatten();
        let Some(selected) = selected else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "choose one published draw replacement",
            ));
        };
        let current =
            work.requests
                .front()
                .zip(work.nodes.last())
                .is_some_and(|(request, node)| {
                    self.draw_candidates(request.player, node)
                        .iter()
                        .any(|(identity, _)| identity == &selected.identity)
                });
        if !current {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("draw replacement source is stale"));
        }
        let identity = selected.identity.clone();
        let action = selected.action;
        let Some(PendingReplacementEvent::Draw(work)) = self.state.pending_replacement_event.take()
        else {
            unreachable!()
        };
        let mut work = *work;
        let ResolutionContinuation::DrawReplacement { stack } = pending.continuation else {
            unreachable!()
        };
        work.completion.transfer_stack(stack);
        let mut events = Vec::new();
        match self.apply_draw_replacement(&mut work, identity, action) {
            AppliedDrawProgress::GameEnded => return Ok(finish_with_events(self, events)),
            AppliedDrawProgress::Parked => {
                self.park_draw_library_action(work, &mut events);
                return Ok(finish_with_events(self, events));
            }
            AppliedDrawProgress::Continue => {}
        }
        match self.advance_draw_transaction(work, &mut events)? {
            DrawProgress::GameEnded => Ok(finish_with_events(self, events)),
            DrawProgress::Parked => Ok(finish_with_events(self, events)),
            DrawProgress::Complete(completed) => self.complete_draw_transaction(completed, events),
        }
    }

    pub(super) fn complete_draw_transaction(
        &mut self,
        completed: Box<CompletedDraw>,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        match completed.completion {
            DrawCompletion::ResumeEffects { stack, mut result } => {
                result.produced_objects.extend(completed.receipts);
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    result,
                    events,
                )
            }
            DrawCompletion::BeginDiscard {
                stack,
                player,
                count,
                mut result,
            } => {
                result.produced_objects.extend(completed.receipts);
                resolution::zones::resume_draw_then_discard(
                    self, stack, player, count, result, events,
                )
            }
            DrawCompletion::FinishDrawStep {
                active_player,
                occurrence,
            } => {
                self.finish_draw_step_action(active_player, occurrence, &mut events)?;
                self.apply_sbas(&mut events)?;
                Ok(finish_with_events(self, events))
            }
            DrawCompletion::BrainstormPutBack {
                stack,
                step,
                scratch,
            } => {
                self.finish_brainstorm_draw(stack.item, step, scratch, &mut events)?;
                if self.state.pending_resolution.is_none() {
                    self.apply_sbas(&mut events)?;
                    events.push(ev_priority_changed(self));
                }
                Ok(finish_with_events(self, events))
            }
        }
    }

    /// An accepted concession can remove the drawer, one replacement source, or the resolving
    /// spell itself. Reevaluate the parked node without applying the vanished choice handle.
    pub(super) fn reconcile_draw_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if !matches!(
            self.state.pending_replacement_event,
            Some(PendingReplacementEvent::Draw(_))
        ) {
            return Ok(());
        }
        let pending = self.state.pending_resolution.take();
        let Some(PendingReplacementEvent::Draw(work)) = self.state.pending_replacement_event.take()
        else {
            unreachable!()
        };
        let Some(pending) = pending.filter(|_| !self.state.is_terminal()) else {
            return Ok(());
        };
        let ResolutionContinuation::DrawReplacement { stack } = pending.continuation else {
            unreachable!()
        };
        let mut work = *work;
        work.completion.transfer_stack(stack);
        work.applications.clear();
        match self.advance_draw_transaction(work, events)? {
            DrawProgress::GameEnded => {}
            DrawProgress::Parked => {}
            DrawProgress::Complete(done) => {
                let batch = self.complete_draw_transaction(done, std::mem::take(events))?;
                *events = batch.events;
            }
        }
        Ok(())
    }

    pub(super) fn finish_draw_step_action(
        &mut self,
        active_player: PlayerId,
        occurrence: u64,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        // Automatic priority settlement bypasses the ordinary command boundary. The
        // turn-based draw has finished, so perform its deferred loss before publishing
        // draw-step triggers or permitting another automatic pass (CR 504.2/704.5b).
        if self
            .state
            .players
            .iter()
            .any(|player| player.id == active_player && player.pending_library_loss)
        {
            events.push(ev_log(if self.state.players.len() == 2 {
                "Game over: empty library on draw".to_string()
            } else {
                format!("P{active_player} loses: empty library on draw")
            }));
        }
        self.commit_pending_library_losses();
        self.sweep_life();
        self.reconcile_departed_players(events)?;
        if self
            .state
            .draw_step_progress
            .is_some_and(|(player, step, _)| player == active_player && step == occurrence)
            && self
                .state
                .players
                .iter()
                .any(|player| player.id == active_player && !player.has_lost)
        {
            self.state.passes_since_stack_change = 0;
            self.fire_triggers(
                &[GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::Draw,
                    active_player,
                }],
                events,
            );
            self.flush_staged_triggers(events);
            if self.state.blocking_choice().is_none() {
                events.push(ev_priority_changed(self));
            }
        }
        Ok(())
    }
}
