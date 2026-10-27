//! Daretti's +2 discards up to two cards, then draws for the committed discards.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

const DARETTI: &str = "daretti,_scrap_savant";

fn setup_daretti(seed: u64) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create game");
    advance_to_main1_from_game_start(&mut engine);
    let daretti = inject_permanent_on_battlefield(&mut engine, 0, DARETTI);
    engine
        .state
        .objects
        .get_mut(&daretti)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    (engine, daretti)
}

fn activate_plus_two(
    engine: &mut GameEngine,
    daretti: u32,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    apply_ability(engine, 0, daretti, 0, vec![]).expect("activate Daretti's +2");
    assert_eq!(
        engine.state.objects[&daretti].counter_count(CounterKind::Loyalty),
        5,
        "loyalty is paid before the ability resolves"
    );
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes");
    engine
        .apply_scenario_command(1, &pass())
        .expect("opponent passes")
}

#[test]
fn daretti_plus_two_discards_zero_one_or_two_and_draws_that_many() {
    for count in 0..=2 {
        let (mut engine, daretti) = setup_daretti(261_027_000 + count);
        let initial_hand = engine.state.players[0].hand.clone();
        let initial_library = engine.state.players[0].library.clone();
        let initial_graveyard = engine.state.players[0].graveyard.len();
        let choice_batch = activate_plus_two(&mut engine, daretti);
        let choice = find_resolution_choice(&choice_batch).expect("private discard choice");
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
        assert_eq!((choice.min, choice.max), (0, 2));
        assert_eq!(
            engine.state.players[0].hand, initial_hand,
            "draw waits for the discard"
        );
        assert_eq!(engine.state.players[0].library, initial_library);

        let before_rejection = format!("{:?}", engine.state);
        engine
            .apply_scenario_command(1, &submit_resolution_choice(vec![]))
            .expect_err("another player cannot choose the discard");
        assert_eq!(format!("{:?}", engine.state), before_rejection);
        engine
            .apply_scenario_command(
                0,
                &submit_resolution_choice(choice.candidate_object_ids[..3].to_vec()),
            )
            .expect_err("choosing more than two is illegal");
        assert_eq!(format!("{:?}", engine.state), before_rejection);

        let discarded = choice.candidate_object_ids[..count as usize].to_vec();
        engine
            .apply_scenario_command(0, &submit_resolution_choice(discarded.clone()))
            .expect("discard the chosen number of cards");
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[0].hand.len(), initial_hand.len());
        assert_eq!(
            engine.state.players[0].library.len(),
            initial_library.len() - count as usize
        );
        assert_eq!(
            engine.state.players[0].graveyard.len(),
            initial_graveyard + count as usize
        );
        for object in discarded {
            assert_eq!(engine.state.objects[&object].zone, Zone::Graveyard);
        }
        for object in initial_library.iter().take(count as usize) {
            assert_eq!(engine.state.objects[object].zone, Zone::Hand);
        }
    }
}

#[test]
fn daretti_plus_two_with_empty_hand_finishes_without_a_choice_or_draw() {
    let (mut engine, daretti) = setup_daretti(261_027_010);
    for object in std::mem::take(&mut engine.state.players[0].hand) {
        engine.state.objects.get_mut(&object).unwrap().zone = Zone::Graveyard;
        engine.state.players[0].graveyard.push(object);
    }
    let initial_library = engine.state.players[0].library.clone();
    let batch = activate_plus_two(&mut engine, daretti);
    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0].hand.is_empty());
    assert_eq!(engine.state.players[0].library, initial_library);
}

#[test]
fn daretti_plus_two_with_one_card_can_discard_and_draw_one() {
    let (mut engine, daretti) = setup_daretti(261_027_011);
    while engine.state.players[0].hand.len() > 1 {
        let object = engine.state.players[0].hand.pop().unwrap();
        engine.state.objects.get_mut(&object).unwrap().zone = Zone::Graveyard;
        engine.state.players[0].graveyard.push(object);
    }
    let discarded = engine.state.players[0].hand[0];
    let drawn = *engine.state.players[0].library.front().unwrap();
    let batch = activate_plus_two(&mut engine, daretti);
    let choice = find_resolution_choice(&batch).expect("up-to-one discard choice");
    assert_eq!((choice.min, choice.max), (0, 1));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard the one card");
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand, vec![drawn]);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn daretti_plus_two_counts_a_library_of_leng_discard_as_one() {
    let (mut engine, daretti) = setup_daretti(261_027_012);
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let discarded = engine.state.players[0].hand[0];
    let original_hand = engine.state.players[0].hand.len();
    let original_library = engine.state.players[0].library.len();
    let generation = engine
        .state
        .zone_change_generation
        .get(&discarded)
        .copied()
        .unwrap_or(0);
    let batch = activate_plus_two(&mut engine, daretti);
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&discarded));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("choose the card to discard");
    let replacement = find_resolution_choice(&batch).expect("Library of Leng destination choice");
    assert_eq!(replacement.choice_kind(), ChoiceKind::PrivateReplacement);
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Hand);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![1]))
        .expect("put the discarded card on top of the library");
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation[&discarded],
        generation + 2
    );
    assert_eq!(engine.state.players[0].hand.len(), original_hand);
    assert_eq!(engine.state.players[0].library.len(), original_library);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

fn graveyard_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

fn select_sacrifice_branch() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn resolve_minus_two(engine: &mut GameEngine, daretti: u32, target: u32) -> RuledEventBatch {
    apply_ability(engine, 0, daretti, 1, graveyard_target(target)).expect("activate Daretti's -2");
    assert_eq!(
        engine.state.objects[&daretti].counter_count(CounterKind::Loyalty),
        1
    );
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes");
    engine
        .apply_scenario_command(1, &pass())
        .expect("opponent passes")
}

fn choose_mandatory_sacrifice(engine: &mut GameEngine, batch: &RuledEventBatch) -> RuledEventBatch {
    let branch = find_resolution_choice(batch).expect("mandatory sacrifice branch");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (1, 1));
    let mut decline = select_sacrifice_branch();
    if let Some(Cmd::SubmitResolutionChoice(answer)) = decline.cmd.as_mut() {
        answer.decision = ResolutionChoiceDecision::Decline as i32;
    }
    engine
        .apply_scenario_command(0, &decline)
        .expect_err("sacrifice cannot be declined");
    engine
        .apply_scenario_command(0, &select_sacrifice_branch())
        .expect("choose mandatory sacrifice")
}

#[test]
fn daretti_minus_two_targets_first_then_sacrifices_a_controlled_artifact() {
    for redirected in [false, true] {
        let (mut engine, daretti) = setup_daretti(261_027_020 + u64::from(redirected));
        let sacrifice = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
        let target = inject_graveyard_card(&mut engine, 0, "mind_stone");
        let foreign = inject_graveyard_card(&mut engine, 1, "sol_ring");
        let nonartifact = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
        if redirected {
            engine
                .state
                .objects
                .get_mut(&sacrifice)
                .unwrap()
                .set_counter(CounterKind::Finality, 1);
        }
        for invalid in [sacrifice, foreign, nonartifact] {
            apply_ability(&mut engine, 0, daretti, 1, graveyard_target(invalid))
                .expect_err("target must be an artifact card already in your graveyard");
            assert_eq!(
                engine.state.objects[&daretti].counter_count(CounterKind::Loyalty),
                3
            );
            assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Battlefield);
        }
        let branch = resolve_minus_two(&mut engine, daretti, target);
        assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
        let payment = choose_mandatory_sacrifice(&mut engine, &branch);
        let choice = find_resolution_choice(&payment).expect("choose artifact to sacrifice");
        assert_eq!((choice.min, choice.max), (1, 1));
        assert!(choice.candidate_object_ids.contains(&sacrifice));
        assert!(!choice.candidate_object_ids.contains(&opponent_artifact));
        let before = format!("{:?}", engine.state);
        for invalid in [vec![], vec![opponent_artifact]] {
            engine
                .apply_scenario_command(0, &submit_resolution_choice(invalid))
                .expect_err("mandatory sacrifice rejects invalid selection");
            assert_eq!(format!("{:?}", engine.state), before);
        }
        engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![sacrifice]))
            .expect("sacrifice the selected artifact");
        assert_eq!(
            engine.state.objects[&sacrifice].zone,
            if redirected {
                Zone::Exile
            } else {
                Zone::Graveyard
            }
        );
        assert_eq!(
            engine.state.objects[&target].zone,
            Zone::Battlefield,
            "a sacrifice redirected to exile still satisfies if you do"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn daretti_minus_two_does_nothing_when_there_is_no_artifact_to_sacrifice() {
    let (mut engine, daretti) = setup_daretti(261_027_022);
    let target = inject_graveyard_card(&mut engine, 0, "mind_stone");
    let batch = resolve_minus_two(&mut engine, daretti, target);
    assert!(find_resolution_choice(&batch).is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn daretti_minus_two_with_a_stale_target_does_not_sacrifice() {
    let (mut engine, daretti) = setup_daretti(261_027_023);
    let sacrifice = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let target = inject_graveyard_card(&mut engine, 0, "mind_stone");
    apply_ability(&mut engine, 0, daretti, 1, graveyard_target(target)).expect("activate -2");
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 2;
    engine.apply_scenario_command(0, &pass()).unwrap();
    let batch = engine.apply_scenario_command(1, &pass()).unwrap();
    assert!(find_resolution_choice(&batch).is_none());
    assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn daretti_minus_two_resumes_after_the_returned_artifacts_copy_choice() {
    let (mut engine, daretti) = setup_daretti(261_027_024);
    let sacrifice = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let donor = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let incoming = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let branch = resolve_minus_two(&mut engine, daretti, incoming);
    let payment = choose_mandatory_sacrifice(&mut engine, &branch);
    assert!(find_resolution_choice(&payment)
        .unwrap()
        .candidate_object_ids
        .contains(&sacrifice));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![sacrifice]))
        .expect("sacrifice before the copy choice");
    let copy = find_resolution_choice(&batch).expect("Sculpting Steel chooses a copy on entry");
    assert_eq!(copy.choice_kind(), ChoiceKind::CopySource);
    assert!(copy.candidate_object_ids.contains(&donor));
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![donor]))
        .expect("copy the opponent's artifact");
    assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].controller, 0);
    assert!(engine
        .characteristics(incoming)
        .unwrap()
        .names
        .iter()
        .any(|name| name == "Short Sword"));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn daretti_minus_two_finishes_when_a_copied_aura_cannot_enter() {
    use tricerules_cards::{
        ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
    };
    use tricerules_core::{AffectedScope, AttachmentRecipient, ContinuousEffect};

    let (mut engine, daretti) = setup_daretti(261_027_026);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "great_furnace");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "gift_of_paradise");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(land));
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(aura),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Artifact],
            ..Default::default()
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    let incoming = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let generation = engine
        .state
        .zone_change_generation
        .get(&incoming)
        .copied()
        .unwrap_or(0);
    let branch = resolve_minus_two(&mut engine, daretti, incoming);
    choose_mandatory_sacrifice(&mut engine, &branch);
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![land]))
        .expect("sacrifice the only land while the ability resolves");
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&aura));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![aura]))
        .expect("choose an artifact Aura with no remaining legal land to enchant");
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&incoming)
            .copied()
            .unwrap_or(0),
        generation
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn daretti_graveyard_entry_program_resumes_its_tail_once_after_copy_choice() {
    const FIXTURE: &str = r#"(
        id: "graveyard_return_tail_fixture",
        name: "Graveyard Return Tail Fixture",
        face_id: "graveyard_return_tail_fixture",
        mana_cost: "{0}",
        types: ["Artifact"],
        activated_abilities: [(
            ability_id: "activated_01", presentation: Fallback,
            costs: [], effect: [],
            targeting: Some((groups: [(
                min: 1, max: 1, prompt: "Choose target artifact in your graveyard",
                effect_indices: [],
            )])),
            program: Some((body: (steps: [
                (id: "choose_sacrifice", kind: Choose(
                    choice: "sacrifice", chooser: Controller, optional: false,
                    branches: [(
                        branch_id: "sacrifice", presentation: Fallback,
                        cost: SacrificePermanent(filter: (
                            kind: AnyPermanent, controller: You, permanent_types: [Artifact],
                        )),
                        body: (steps: [
                            (id: "return", kind: Perform(MoveGraveyardCards(
                                filter: (card: Some((card_type: Some(Artifact)))),
                                destination: Battlefield(),
                            )))
                        ]),
                    )], otherwise: (),
                )),
                (id: "tail", kind: Perform(GainLife(amount: 1))),
            ]), target_bindings: [
                (instruction_id: "return", role: Target, target_group_index: 0),
            ])),
        )],
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                FIXTURE,
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/sol_ring.ron"),
                include_str!("../../../tricerules-cards/data/short_sword.ron"),
                include_str!("../../../tricerules-cards/data/sculpting_steel.ron"),
            ],
            &[],
        )
        .expect("targeted graveyard-to-battlefield Program fixture compiles"),
    ));
    for (label, from, to) in [
        (
            "optional target",
            "min: 1, max: 1, prompt:",
            "min: 0, max: 1, prompt:",
        ),
        (
            "multiple targets",
            "min: 1, max: 1, prompt:",
            "min: 1, max: 2, prompt:",
        ),
        (
            "foreign graveyard",
            "filter: (card: Some((card_type: Some(Artifact)))),",
            "filter: (owner: AnyPlayer, card: Some((card_type: Some(Artifact)))),",
        ),
        (
            "creature target",
            "card_type: Some(Artifact)",
            "card_type: Some(Creature)",
        ),
        (
            "tapped return",
            "destination: Battlefield()",
            "destination: Battlefield(tapped: true)",
        ),
        (
            "bound result",
            "(id: \"return\", kind:",
            "(id: \"return\", bind: [\"result\"], kind:",
        ),
    ] {
        let invalid = FIXTURE.replace(from, to);
        assert_ne!(invalid, FIXTURE, "{label} mutation applies");
        assert!(
            tricerules_cards::CardRegistry::from_chunks_and_tokens(&[&invalid], &[]).is_err(),
            "{label} is outside the bounded graveyard return profile"
        );
    }
    let mut engine = GameEngine::new(
        registry,
        261_027_025,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create fixture game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "graveyard_return_tail_fixture");
    let sacrifice = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let donor = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let incoming = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let life_before = engine.state.players[0].life;
    apply_ability(&mut engine, 0, source, 0, graveyard_target(incoming)).expect("activate fixture");
    engine.apply_scenario_command(0, &pass()).unwrap();
    let branch = engine.apply_scenario_command(1, &pass()).unwrap();
    let payment = choose_mandatory_sacrifice(&mut engine, &branch);
    assert!(find_resolution_choice(&payment)
        .unwrap()
        .candidate_object_ids
        .contains(&sacrifice));
    let batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![sacrifice]))
        .expect("sacrifice and begin entry");
    let copy = find_resolution_choice(&batch).expect("copy choice parks the entry");
    assert_eq!(copy.choice_kind(), ChoiceKind::CopySource);
    assert!(copy.candidate_object_ids.contains(&donor));
    assert_eq!(
        engine.state.players[0].life, life_before,
        "tail waits for the entry"
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![donor]))
        .expect("complete the entry and resume the Program");
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.players[0].life,
        life_before + 1,
        "the tail executes exactly once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

fn create_daretti_emblem(engine: &mut GameEngine, daretti: u32) -> u32 {
    engine
        .state
        .objects
        .get_mut(&daretti)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 10);
    apply_ability(engine, 0, daretti, 2, vec![]).expect("activate Daretti's -10");
    pass_both_players(engine);
    assert_eq!(
        engine.state.objects[&daretti].zone,
        Zone::Graveyard,
        "Daretti leaves with zero loyalty even though the ability resolves"
    );
    let marker = engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == 0)
        .expect("the ultimate creates an emblem");
    let marker_id = marker.object_id;
    assert!(engine.initial_response_batch().events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::ZoneView(view)) if view.per_player.iter().any(|player| {
            player.player_id == 0 && player.static_emblems.iter().any(|emblem| emblem.object_id == marker_id)
        }))
    }), "the existing public player view publishes the emblem marker");
    marker_id
}

fn dev_move_owned_card(
    engine: &mut GameEngine,
    owner: i32,
    name: &str,
    zone: tricerules_proto::ruled::v1::DevZone,
) -> RuledEventBatch {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard};
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the physical card through the engine")
}

fn advance_to_end_step(engine: &mut GameEngine) -> RuledEventBatch {
    let mut batch = RuledEventBatch::default();
    for _ in 0..10 {
        if engine.state.turn_step == tricerules_core::TurnStep::EndStep {
            break;
        }
        let active = engine.state.active_player_id();
        batch = engine
            .apply_scenario_command(active, &primitive_yield())
            .expect("advance to end step");
    }
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    batch
}

fn assert_emblem_stack_presentation(
    batch: &RuledEventBatch,
    object_id: u32,
    expected_path: &[&str],
) {
    use tricerules_proto::ruled::v1::PresentationPathKind;
    let pushed = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(pushed)) if pushed.object_id == object_id => Some(pushed),
            _ => None,
        })
        .expect("the trigger's public stack event is emitted");
    assert!(pushed.is_triggered);
    let presentation = pushed
        .primary_presentation
        .as_ref()
        .expect("captured presentation");
    assert_eq!(presentation.card_id, DARETTI);
    assert_eq!(presentation.face_id, "daretti_scrap_savant");
    assert_eq!(presentation.oracle_line_indices, [3]);
    assert_eq!(
        presentation
            .path
            .iter()
            .map(|part| part.id.as_str())
            .collect::<Vec<_>>(),
        expected_path
    );
    assert!(presentation
        .path
        .iter()
        .all(|part| part.kind == PresentationPathKind::Ability as i32));
}

#[test]
fn daretti_emblem_stacks_a_trigger_then_returns_the_exact_card_at_the_next_end_step() {
    use tricerules_proto::ruled::v1::DevZone;
    let (mut engine, daretti) = setup_daretti(261_027_030);
    let marker = create_daretti_emblem(&mut engine, daretti);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let generation = engine
        .state
        .zone_change_generation
        .get(&artifact)
        .copied()
        .unwrap_or(0);
    let trigger_batch = dev_move_owned_card(&mut engine, 0, "Sol Ring", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the emblem ability uses the stack"
    );
    assert!(engine.state.stack[0].is_triggered);
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(marker));
    assert_emblem_stack_presentation(
        &trigger_batch,
        engine.state.stack[0].id,
        &["activated_03", "emblem_01"],
    );
    assert!(
        engine.state.active_event_observers.is_empty(),
        "the initial trigger can be countered"
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    let delayed_batch = advance_to_end_step(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the delayed return also uses the stack"
    );
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(marker));
    assert_emblem_stack_presentation(
        &delayed_batch,
        engine.state.stack[0].id,
        &["activated_03", "emblem_01", "delayed_01"],
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&artifact].controller, 0);
    assert_eq!(
        engine.state.zone_change_generation[&artifact],
        generation + 2
    );
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn daretti_emblem_never_returns_a_later_graveyard_incarnation() {
    use tricerules_proto::ruled::v1::DevZone;
    for leave_before_initial_trigger_resolves in [true, false] {
        let (mut engine, daretti) =
            setup_daretti(261_027_031 + u64::from(leave_before_initial_trigger_resolves));
        create_daretti_emblem(&mut engine, daretti);
        let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        dev_move_owned_card(&mut engine, 0, "Sol Ring", DevZone::Graveyard);
        if !leave_before_initial_trigger_resolves {
            pass_both_players(&mut engine);
        }
        dev_move_owned_card(&mut engine, 0, "Sol Ring", DevZone::Hand);
        dev_move_owned_card(&mut engine, 0, "Sol Ring", DevZone::Graveyard);
        let later_generation = engine.state.zone_change_generation[&artifact];
        if leave_before_initial_trigger_resolves {
            pass_both_players(&mut engine);
        }
        assert_eq!(
            engine.state.active_event_observers.len(),
            1,
            "a resolved initial trigger creates its delayed ability even if the card has left"
        );
        advance_to_end_step(&mut engine);
        assert_eq!(engine.state.stack.len(), 1);
        pass_both_players(&mut engine);
        assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.zone_change_generation[&artifact],
            later_generation
        );
    }
}

fn destroy_artifact(engine: &mut GameEngine, object: u32) {
    inject_card_into_hand(engine, 0, "ancient_grudge");
    give_mana(
        engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "ancient_grudge");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(object)))
        .expect("cast Ancient Grudge");
    pass_both_players(engine);
}

#[test]
fn daretti_emblem_uses_the_artifacts_owner_and_its_last_battlefield_types() {
    use tricerules_cards::{
        ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
    };
    use tricerules_core::{AffectedScope, ContinuousEffect};
    let (mut engine, daretti) = setup_daretti(261_027_033);
    create_daretti_emblem(&mut engine, daretti);
    let foreign = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let owned = inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    for (object, old_seat, new_seat) in [(foreign, 1, 0), (owned, 0, 1)] {
        engine.state.players[old_seat]
            .battlefield
            .retain(|oid| *oid != object);
        engine.state.players[new_seat].battlefield.push(object);
        let permanent = engine.state.objects.get_mut(&object).unwrap();
        permanent.base_controller = new_seat as i32;
        permanent.controller = new_seat as i32;
    }
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(owned),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Artifact],
            ..Default::default()
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    destroy_artifact(&mut engine, foreign);
    assert_eq!(engine.state.objects[&foreign].zone, Zone::Graveyard);
    assert!(
        engine.state.stack.is_empty(),
        "a controlled artifact owned by the opponent does not trigger"
    );
    destroy_artifact(&mut engine, owned);
    assert_eq!(engine.state.objects[&owned].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "your artifact triggers even under an opponent's control"
    );
    pass_both_players(&mut engine);
    advance_to_end_step(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&owned].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&owned].controller, 0);
}

#[test]
fn daretti_emblem_token_triggers_but_cannot_return_a_card() {
    let (mut engine, daretti) = setup_daretti(261_027_034);
    create_daretti_emblem(&mut engine, daretti);
    let treasure = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
    apply_ability(&mut engine, 0, treasure, 0, vec![]).expect("sacrifice the Treasure for mana");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "an artifact token still produces the emblem trigger"
    );
    pass_both_players(&mut engine);
    advance_to_end_step(&mut engine);
    pass_both_players(&mut engine);
    assert!(
        !engine.state.objects.contains_key(&treasure),
        "the ceased token cannot return"
    );
}

fn counter_trigger_with_tidebinder(engine: &mut GameEngine, trigger_id: u32) {
    if engine.state.priority_player_id() == 0 {
        engine
            .apply_scenario_command(0, &pass())
            .expect("pass priority to the opponent");
    }
    inject_card_into_hand(engine, 1, "tishanas_tidebinder");
    give_mana(
        engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast Tidebinder in response");
    pass_both_players(engine);
    let target = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id: trigger_id,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(1, &target)
        .expect("target the emblem's triggered ability");
    pass_both_players(engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == trigger_id));
}

#[test]
fn daretti_emblem_initial_and_delayed_triggers_can_each_be_countered() {
    use tricerules_proto::ruled::v1::DevZone;
    for counter_delayed in [false, true] {
        let (mut engine, daretti) = setup_daretti(261_027_035 + u64::from(counter_delayed));
        create_daretti_emblem(&mut engine, daretti);
        let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        dev_move_owned_card(&mut engine, 0, "Sol Ring", DevZone::Graveyard);
        if counter_delayed {
            pass_both_players(&mut engine);
            advance_to_end_step(&mut engine);
        }
        let trigger_id = engine
            .state
            .stack
            .last()
            .expect("emblem trigger on stack")
            .id;
        counter_trigger_with_tidebinder(&mut engine, trigger_id);
        if !counter_delayed {
            assert!(engine.state.active_event_observers.is_empty());
            advance_to_end_step(&mut engine);
        }
        assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
        assert!(engine.state.active_event_observers.is_empty());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            engine.state.static_emblems.len(),
            1,
            "countering a trigger does not remove the emblem"
        );
    }
}

#[test]
fn daretti_emblem_trigger_created_during_end_step_waits_for_the_next_end_step() {
    use tricerules_proto::ruled::v1::DevZone;
    let (mut engine, daretti) = setup_daretti(261_027_040);
    create_daretti_emblem(&mut engine, daretti);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    advance_to_end_step(&mut engine);
    let original_turn = engine.state.turn_instance;
    dev_move_owned_card(&mut engine, 0, "Sol Ring", DevZone::Graveyard);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("finish the current end step");
    assert!(engine.state.turn_instance > original_turn);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn daretti_emblem_ignores_artifacts_exiled_instead_of_entering_graveyard() {
    let (mut engine, daretti) = setup_daretti(261_027_041);
    create_daretti_emblem(&mut engine, daretti);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    engine
        .state
        .objects
        .get_mut(&artifact)
        .unwrap()
        .set_counter(CounterKind::Finality, 1);
    destroy_artifact(&mut engine, artifact);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.active_event_observers.is_empty());
    advance_to_end_step(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Exile);
}

#[test]
fn daretti_emblem_delayed_return_pauses_for_a_copy_entry_choice() {
    use tricerules_proto::ruled::v1::DevZone;
    let (mut engine, daretti) = setup_daretti(261_027_042);
    create_daretti_emblem(&mut engine, daretti);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sculpting_steel");
    dev_move_owned_card(&mut engine, 0, "Sculpting Steel", DevZone::Graveyard);
    pass_both_players(&mut engine);
    let donor = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    advance_to_end_step(&mut engine);
    let batch = pass_both_players_with_batch(&mut engine);
    let choice = find_resolution_choice(&batch).expect("the returned Steel chooses a copy");
    assert_eq!(choice.choice_kind(), ChoiceKind::CopySource);
    assert!(choice.candidate_object_ids.contains(&donor));
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![donor]))
        .expect("complete the delayed return");
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&artifact].controller, 0);
    assert!(engine
        .characteristics(artifact)
        .unwrap()
        .names
        .iter()
        .any(|name| name == "Short Sword"));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn daretti_emblem_event_and_delayed_return_replay_accepted_wire_commands() {
    use prost::Message;
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

    fn fixture() -> (GameEngine, u32) {
        let (mut engine, daretti) = setup_daretti(261_027_043);
        create_daretti_emblem(&mut engine, daretti);
        let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        engine.enable_dev_commands();
        (engine, artifact)
    }
    let (mut engine, artifact) = fixture();
    let mut commands = Vec::new();
    let mut batches = Vec::new();
    let mv = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: "Sol Ring".into(),
                zone: DevZone::Graveyard as i32,
                ready: false,
            })),
        })),
    };
    let actor = engine.state.priority_player_id();
    batches.push(engine.apply_scenario_command(actor, &mv).unwrap());
    commands.push((actor, mv));
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        let cmd = pass();
        batches.push(engine.apply_scenario_command(actor, &cmd).unwrap());
        commands.push((actor, cmd));
    }
    while engine.state.turn_step != tricerules_core::TurnStep::EndStep {
        let actor = engine.state.active_player_id();
        let cmd = primitive_yield();
        batches.push(engine.apply_scenario_command(actor, &cmd).unwrap());
        commands.push((actor, cmd));
    }
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        let cmd = pass();
        batches.push(engine.apply_scenario_command(actor, &cmd).unwrap());
        commands.push((actor, cmd));
    }
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    let (mut replay, replay_artifact) = fixture();
    assert_eq!(artifact, replay_artifact);
    for ((actor, command), expected) in commands.into_iter().zip(batches) {
        let bytes = command.encode_to_vec();
        let decoded = RuledCommand::decode(bytes.as_slice()).unwrap();
        assert_eq!(
            replay.apply_scenario_command(actor, &decoded).unwrap(),
            expected
        );
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

fn create_emblem_for_active_player(engine: &mut GameEngine, seat: usize) -> u32 {
    let player = engine.state.players[seat].id;
    assert_eq!(engine.state.active_player_id(), player);
    let daretti = inject_permanent_on_battlefield(engine, seat, DARETTI);
    engine
        .state
        .objects
        .get_mut(&daretti)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 10);
    let existing: Vec<_> = engine
        .state
        .static_emblems
        .iter()
        .map(|emblem| emblem.object_id)
        .collect();
    apply_ability(engine, player, daretti, 2, vec![]).unwrap();
    pass_priority_round(engine);
    engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == player && !existing.contains(&emblem.object_id))
        .expect("new player's emblem exists")
        .object_id
}

#[test]
fn daretti_emblems_order_simultaneous_events_apnap_and_each_returns_at_most_once() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        261_027_044,
        &[0, 1],
        20,
        Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let first = create_emblem_for_active_player(&mut engine, 0);
    let second = create_emblem_for_active_player(&mut engine, 0);
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.active_player_id(), 1);
    while engine.state.turn_step != tricerules_core::TurnStep::Main1 {
        engine
            .apply_scenario_command(1, &primitive_yield())
            .unwrap();
    }
    let active_marker = create_emblem_for_active_player(&mut engine, 1);
    let p0_artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let p1_artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    inject_card_into_hand(&mut engine, 1, "shatterstorm");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "shatterstorm");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&p0_artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&p1_artifact].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "active player's trigger is placed first"
    );
    assert_eq!(
        engine.state.stack[0].source_permanent_id,
        Some(active_marker)
    );
    let pending = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("nonactive player orders two triggers");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.candidates.len(), 2);
    answer_trigger_order_in_engine_order(&mut engine);
    let sources: Vec<_> = engine
        .state
        .stack
        .iter()
        .map(|item| item.source_permanent_id.unwrap())
        .collect();
    assert_eq!(sources[0], active_marker);
    assert!(sources[1..].contains(&first) && sources[1..].contains(&second));
    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.active_event_observers.len(), 3);
    advance_to_end_step(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "active player's delayed trigger is placed first"
    );
    assert_eq!(
        engine.state.stack[0].source_permanent_id,
        Some(active_marker)
    );
    let pending = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("order two delayed triggers");
    assert_eq!(pending.deciding_player, 0);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 3);
    while !engine.state.stack.is_empty() {
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.objects[&p0_artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&p1_artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&p0_artifact].controller, 0);
    assert_eq!(engine.state.objects[&p1_artifact].controller, 1);
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn daretti_emblem_and_its_pending_triggers_leave_with_their_controller() {
    use tricerules_proto::ruled::v1::DevZone;
    for resolve_initial in [false, true] {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            261_027_045 + u64::from(resolve_initial),
            &[0, 1, 2],
            20,
            Some(vec![
                deck_with("mountain", &[]),
                deck_with("forest", &[]),
                deck_with("island", &[]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let marker = create_emblem_for_active_player(&mut engine, 0);
        inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        dev_move_owned_card(&mut engine, 0, "Sol Ring", DevZone::Graveyard);
        if resolve_initial {
            pass_priority_round(&mut engine);
            assert_eq!(engine.state.active_event_observers.len(), 1);
        } else {
            assert_eq!(engine.state.stack.len(), 1);
        }
        engine
            .apply_scenario_command(0, &concede())
            .expect("emblem controller concedes");
        assert!(!engine
            .state
            .static_emblems
            .iter()
            .any(|emblem| emblem.object_id == marker));
        assert!(engine.state.active_event_observers.is_empty());
        assert!(!engine
            .state
            .stack
            .iter()
            .any(|item| item.source_permanent_id == Some(marker)));
        assert!(engine.state.pending_trigger_order.is_none());
        assert!(engine.state.pending_triggers.is_empty());
    }
}

#[test]
fn daretti_emblem_survives_departure_of_the_original_planeswalkers_owner() {
    use tricerules_proto::ruled::v1::DevZone;
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        261_027_046,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("mountain", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 0);
    while engine.state.turn_step != tricerules_core::TurnStep::Main1 {
        engine
            .apply_scenario_command(1, &primitive_yield())
            .unwrap();
    }
    let daretti = inject_permanent_on_battlefield(&mut engine, 0, DARETTI);
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != daretti);
    engine.state.players[1].battlefield.push(daretti);
    let permanent = engine.state.objects.get_mut(&daretti).unwrap();
    permanent.base_controller = 1;
    permanent.controller = 1;
    permanent.set_counter(CounterKind::Loyalty, 10);
    engine.initial_response_batch();
    apply_ability(&mut engine, 1, daretti, 2, vec![]).unwrap();
    pass_priority_round(&mut engine);
    let marker = engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == 1)
        .unwrap()
        .object_id;
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    dev_move_owned_card(&mut engine, 1, "Short Sword", DevZone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    engine.apply_scenario_command(0, &concede()).unwrap();
    assert!(engine
        .state
        .static_emblems
        .iter()
        .any(|emblem| emblem.object_id == marker));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the surviving controller retains the trigger"
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    advance_to_end_step(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&artifact].controller, 1);
}

#[test]
fn daretti_triggered_emblem_admission_is_bounded_to_its_captured_ability_scope() {
    use tricerules_cards::EffectContext;
    let source = include_str!("../../../tricerules-cards/data/daretti,_scrap_savant.ron");
    let registry = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[source], &[])
        .expect("the nested trigger object belongs to the emblem child, not its parent activation");
    let card = registry.get(DARETTI).unwrap();
    let effect = &card.primary_face().activated_abilities[2].effect[0];
    assert!(
        effect.validate(EffectContext::Spell).is_err(),
        "this profile requires an ability source"
    );
    for (label, from, to) in [
        ("opponent's graveyard", "owner: Some(Controller)", "owner: Some(Opponent)"),
        ("exile destination", "destination: OneOf([Graveyard])", "destination: OneOf([Exile])"),
        ("batch trigger", "cardinality: EachObject", "cardinality: OneOrMore"),
        ("wrong watched subject", "subject: Some(TriggerObject)", "subject: Some(Source)"),
        ("non-exact return", "reference: ExactTriggerObject", "reference: TriggerObject"),
        ("return under owner", "controller: AbilityController", "controller: Owner"),
        ("unowned child program", "ability_id: \"emblem_01\",", "ability_id: \"emblem_01\", program: Some((body: (steps: [(id: \"draw\", kind: Perform(Draw(count: 1)))]))),"),
        ("unowned delayed program", "ability_id: \"delayed_01\",", "ability_id: \"delayed_01\", program: Some((body: (steps: [(id: \"draw\", kind: Perform(Draw(count: 1)))]))),"),
        ("empty emblem id", "emblem_id: \"daretti_scrap_savant_emblem\"", "emblem_id: \"\""),
    ] {
        let invalid = source.replace(from, to);
        assert_ne!(invalid, source, "{label} mutation applies");
        assert!(tricerules_cards::CardRegistry::from_chunks_and_tokens(&[&invalid], &[]).is_err(),
            "{label} lies outside the admitted emblem profile");
    }
}

#[test]
fn daretti_is_accepted_as_a_declared_commander_and_as_a_mainboard_card() {
    use tricerules_cards::Color;
    use tricerules_core::EngineDeck;
    let engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        261_027_047,
        &[0, 1],
        20,
        Some(vec![
            EngineDeck {
                mainboard: deck_with("mountain", &[DARETTI]),
                commanders: vec![DARETTI.into()],
            },
            EngineDeck {
                mainboard: deck_with("forest", &[]),
                commanders: vec![],
            },
        ]),
        true,
    )
    .expect("completed Daretti is admitted in both engine deck positions");
    let player = &engine.state.players[0];
    let commander = player.command_zone[0];
    assert_eq!(player.declared_commander_object_ids, vec![commander]);
    assert_eq!(player.color_identity, vec![Color::Red]);
    assert_eq!(engine.state.objects[&commander].card_id, DARETTI);
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert_eq!(
        engine
            .state
            .objects
            .values()
            .filter(|object| object.owner == 0 && object.card_id == DARETTI)
            .count(),
        2,
        "the declared commander and the mainboard copy retain separate physical identities"
    );
}
