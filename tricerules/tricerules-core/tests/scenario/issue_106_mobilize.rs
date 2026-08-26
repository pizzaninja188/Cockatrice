use crate::helpers::*;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CombatDefenderOption, DeclareAttackers, RuledCommand,
};

fn setup_mobilize_with_planeswalker(seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("mountain", &["shock_brigade"]),
        deck_with("island", &["jace_beleren"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut engine);
    let shock = relocate_to_battlefield(&mut engine, 0, "shock_brigade", false);
    let jace = relocate_to_battlefield(&mut engine, 1, "jace_beleren", false);
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(tricerules_cards::CounterKind::Loyalty, 3);
    let assignment = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == shock
                && assignment
                    .defender
                    .as_ref()
                    .is_some_and(|defender| defender.kind == TargetRefKind::Player as i32)
        })
        .cloned()
        .expect("Shock Brigade may attack the opponent");
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .expect("declare Shock Brigade");
    (engine, shock, jace)
}

fn resolve_to_defender_choice(engine: &mut GameEngine) -> Vec<CombatDefenderOption> {
    engine.apply_command(0, &pass()).expect("attacker pass");
    let batch = engine.apply_command(1, &pass()).expect("Mobilize resolves");
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => {
                assert_eq!(
                    choice.choice_kind,
                    ChoiceKind::AttackingTokenDefender as i32
                );
                assert_eq!((choice.min, choice.max), (1, 1));
                Some(choice.combat_defender_options.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "Mobilize defender choice; events={:?}; stack={:?}",
                batch.events, engine.state.stack
            )
        })
}

fn submit_defender(option: CombatDefenderOption) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_combat_defender: Some(option),
            ..Default::default()
        })),
    }
}

#[test]
fn mobilize_enters_tapped_attacking_chosen_planeswalker_and_sacrifices_next_end_step() {
    let (mut engine, _shock, jace) = setup_mobilize_with_planeswalker(106_001);
    let options = resolve_to_defender_choice(&mut engine);
    let jace_option = options
        .iter()
        .find(|option| {
            option
                .defender
                .as_ref()
                .is_some_and(|defender| defender.object_id == jace)
        })
        .cloned()
        .expect("planeswalker defender option");

    let mut stale = jace_option;
    stale.defender_zone_change_generation += 1;
    assert!(engine.apply_command(0, &submit_defender(stale)).is_err());
    assert!(
        engine.state.pending_resolution.is_some(),
        "rejection keeps prompt"
    );

    let resolved = engine
        .apply_command(0, &submit_defender(jace_option))
        .expect("choose Jace for Warrior");
    let tokens = battlefield_token_oids(&engine, 0, "warrior_r_1_1");
    assert_eq!(tokens.len(), 1);
    let token = tokens[0];
    assert!(engine.state.objects[&token].tapped);
    assert!(engine
        .state
        .combat
        .as_ref()
        .unwrap()
        .attacking
        .contains(&token));
    assert!(resolved.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::TokenCreated(created)) if created.object_id == token && created.enters_tapped
    )));
    let added = resolved
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::AttackersAdded(added)) => Some(added),
            _ => None,
        })
        .expect("additive attacker event");
    assert_eq!(added.assignments.len(), 1);
    assert_eq!(added.assignments[0].attacker_object_id, token);
    assert_eq!(
        added.assignments[0].defender.as_ref().unwrap().object_id,
        jace
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);

    for _ in 0..8 {
        if engine.state.turn_step == tricerules_core::TurnStep::EndStep {
            break;
        }
        engine
            .apply_command(0, &primitive_yield())
            .expect("advance toward end step");
    }
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    assert_eq!(engine.state.stack.len(), 1, "one delayed cohort trigger");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        !engine.state.objects.contains_key(&token),
        "the exact Warrior token was sacrificed and ceased to exist"
    );
}

#[test]
fn mobilize_trigger_survives_source_departure() {
    let (mut engine, shock, _jace) = setup_mobilize_with_planeswalker(106_002);
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != shock);
    engine.state.objects.get_mut(&shock).unwrap().zone = tricerules_core::Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(shock)
        .or_default() += 1;

    let options = resolve_to_defender_choice(&mut engine);
    let player_option = options
        .into_iter()
        .find(|option| {
            option.defender.as_ref().is_some_and(|defender| {
                defender.kind == TargetRefKind::Player as i32 && defender.object_id == 1
            })
        })
        .expect("opponent player option");
    engine
        .apply_command(0, &submit_defender(player_option))
        .expect("resolve source-independent Mobilize trigger");
    assert_eq!(battlefield_token_oids(&engine, 0, "warrior_r_1_1").len(), 1);
}
