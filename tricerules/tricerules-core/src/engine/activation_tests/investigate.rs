use super::*;

// Isolated "investigate twice" and "each player investigates" sources (Ezrim, Agency Chief;
// No Witnesses) — not admitted cards.
const TWICE: &str = r#"(
    id: "synthetic_investigate_twice", name: "Synthetic Investigate Twice",
    face_id: "synthetic_investigate_twice", types: ["Artifact"],
    activated_abilities: [(ability_id: "twice", presentation: Fallback,
        costs: [], effect: [Investigate(count: 2)])],
)"#;

const EACH_PLAYER: &str = r#"(
    id: "synthetic_each_player_investigates", name: "Synthetic Each Player Investigates",
    face_id: "synthetic_each_player_investigates", types: ["Artifact"],
    activated_abilities: [(ability_id: "each", presentation: Fallback,
        costs: [], effect: [Investigate(count: 1, who: EachPlayer)])],
)"#;

fn fixture() -> GameEngine {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/orb_of_dreams.ron"),
            TWICE,
            EACH_PLAYER,
        ],
        &[include_str!(
            "../../../../tricerules-cards/data/tokens/clue.ron"
        )],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        339_016,
        &[17, 29],
        20,
        Some(vec![
            EngineDeck {
                mainboard: vec!["forest".into(); 40],
                commanders: vec![],
            };
            2
        ]),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
}

fn activate_and_resolve(engine: &mut GameEngine, source: ObjectId) -> Vec<String> {
    let generation = engine.payment_object_ref(source).zone_change_generation;
    let mut logs = Vec::new();
    let record = |batch: RuledEventBatch, logs: &mut Vec<String>| {
        logs.extend(batch.events.into_iter().filter_map(|event| match event.ev {
            Some(rv1::ruled_event::Ev::Log(log)) => Some(log.text),
            _ => None,
        }));
    };
    let batch = engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        ability_index: 0,
                        expected_zone_change_generation: generation,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    record(batch, &mut logs);
    while !engine.state.stack.is_empty() && engine.state.pending_resolution.is_none() {
        let batch = engine
            .apply_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
        record(batch, &mut logs);
    }
    logs
}

fn choose_first_replacement(engine: &mut GameEngine) -> Vec<String> {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("CR 616 entry replacement choice");
    assert_eq!(
        pending.presentation.choice_kind,
        rv1::ChoiceKind::ReplacementEffect
    );
    let chosen = pending.presentation.candidates[0];
    let decider = pending.deciding_player;
    let batch = engine
        .apply_scenario_command(
            decider,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![chosen],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("choose an entry replacement");
    batch
        .events
        .into_iter()
        .filter_map(|event| match event.ev {
            Some(rv1::ruled_event::Ev::Log(log)) => Some(log.text),
            _ => None,
        })
        .collect()
}

fn clues(engine: &GameEngine, controller: PlayerId) -> Vec<ObjectId> {
    let index = engine.state.player_idx(controller).unwrap();
    engine.state.players[index]
        .battlefield
        .iter()
        .copied()
        .filter(|oid| engine.state.objects[oid].card_id == "clue")
        .collect()
}

fn action_logs(logs: &[String]) -> Vec<&str> {
    logs.iter()
        .map(String::as_str)
        .filter(|line| line.contains("Clue token") || line.contains("investigates"))
        .collect()
}

#[test]
fn investigate_twice_performs_two_separate_clue_creations() {
    let mut engine = fixture();
    let source = place_ready(&mut engine, 17, "Synthetic Investigate Twice");
    let logs = activate_and_resolve(&mut engine, source);
    assert_eq!(
        action_logs(&logs),
        [
            "P17 creates 1 Clue token (Synthetic Investigate Twice).",
            "P17 investigates (Synthetic Investigate Twice).",
            "P17 creates 1 Clue token (Synthetic Investigate Twice).",
            "P17 investigates (Synthetic Investigate Twice).",
        ],
        "each action creates its own Clue and completes before the next: {logs:?}"
    );
    assert_eq!(clues(&engine, 17).len(), 2);
    assert!(clues(&engine, 29).is_empty());
}

#[test]
fn parked_clue_entry_resumes_the_remaining_investigate_action() {
    let mut engine = fixture();
    let source = place_ready(&mut engine, 17, "Synthetic Investigate Twice");
    // Two "enters tapped" replacements make every Clue entry a CR 616.1 order choice. The second
    // Orb enters under only the first, so setup itself needs no choice.
    place_ready(&mut engine, 17, "Orb of Dreams");
    place_ready(&mut engine, 17, "Orb of Dreams");
    assert!(engine.state.pending_resolution.is_none());
    let mut logs = activate_and_resolve(&mut engine, source);
    assert!(
        clues(&engine, 17).is_empty(),
        "the first Clue is still proposed"
    );
    assert!(!logs.iter().any(|line| line.contains("investigates")));

    logs.extend(choose_first_replacement(&mut engine));
    assert_eq!(
        clues(&engine, 17).len(),
        1,
        "the first action completed before the second Clue was proposed"
    );
    assert!(
        engine.state.pending_resolution.is_some(),
        "the second investigate action parks its own Clue entry"
    );

    logs.extend(choose_first_replacement(&mut engine));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    let clues = clues(&engine, 17);
    assert_eq!(
        clues.len(),
        2,
        "resumption must not duplicate or drop a Clue"
    );
    assert!(clues.iter().all(|oid| engine.state.objects[oid].tapped));
    assert_eq!(
        logs.iter()
            .filter(|line| *line == "P17 investigates (Synthetic Investigate Twice).")
            .count(),
        2,
        "{logs:?}"
    );
}

#[test]
fn each_player_investigates_once_in_apnap_order() {
    let mut engine = fixture();
    let source = place_ready(&mut engine, 17, "Synthetic Each Player Investigates");
    let logs = activate_and_resolve(&mut engine, source);
    assert_eq!(
        action_logs(&logs),
        [
            "P17 creates 1 Clue token (Synthetic Each Player Investigates).",
            "P17 investigates (Synthetic Each Player Investigates).",
            "P29 creates 1 Clue token (Synthetic Each Player Investigates).",
            "P29 investigates (Synthetic Each Player Investigates).",
        ],
        "{logs:?}"
    );
    assert_eq!(clues(&engine, 17).len(), 1);
    assert_eq!(clues(&engine, 29).len(), 1);
}
