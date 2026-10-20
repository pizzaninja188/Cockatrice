//! Current-admitted Program cursor regression for counter-placement replacement choices.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChoiceKind, RuledCommand};

const PROGRAM_COUNTER_TAIL: &str = r#"(
    id: "issue_507_counter_tail_fixture",
    name: "Issue 507 Counter Tail Fixture",
    face_id: "issue_507_counter_tail_fixture",
    mana_cost: "{0}",
    types: ["Creature"],
    power: 1,
    toughness: 1,
    triggered_abilities: [(
        ability_id: "triggered_01",
        presentation: Fallback,
        trigger: WhenSelfEntersBattlefield,
        effect: [],
        program: Some((body: (steps: [
            (id: "put_counter", kind: Perform(PutCounters(
                counter: PlusOnePlusOne, count: 1, subject: Source,
            ))),
            (id: "tail_life", kind: Perform(GainLife(amount: 1))),
        ], yields: []), target_bindings: [])),
    )],
)"#;

fn reject_without_state_change(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().expect("diagnostic snapshot");
    assert!(engine.apply_scenario_command(actor, command).is_err());
    assert_eq!(
        engine.diagnostic_snapshot().expect("diagnostic snapshot"),
        before,
        "a rejected replacement-order command leaves the pending Program untouched"
    );
}

#[test]
fn issue_507_counter_placement_replacement_resumes_program_tail_once() {
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                PROGRAM_COUNTER_TAIL,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/doubling_season.ron"),
            ],
            &[],
        )
        .expect("the admitted Source +1/+1 counter Program fixture compiles"),
    ));
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(registry, 507_001, &[0, 1], 20, decks, true)
        .expect("counter-tail fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_card_into_hand(&mut engine, 0, "issue_507_counter_tail_fixture");
    let slot = hand_index_for_card(&engine, 0, "issue_507_counter_tail_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the zero-cost Program fixture");
    pass_priority_round(&mut engine);

    let source = engine
        .state
        .objects
        .values()
        .find(|object| {
            object.card_id == "issue_507_counter_tail_fixture" && object.zone == Zone::Battlefield
        })
        .expect("fixture creature resolved")
        .id;
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| { item.source_permanent_id == Some(source) && item.is_triggered }));
    let batch = pass_both_players_with_batch(&mut engine);
    let replacement = find_resolution_choice(&batch)
        .expect("two Doubling Seasons pause the counter placement for replacement order");
    assert_eq!(
        replacement.choice_kind,
        ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(replacement.deciding_player_id, 0);
    assert_eq!(replacement.candidate_object_ids.len(), 2);

    let answer = submit_resolution_choice(vec![replacement.candidate_object_ids[0]]);
    reject_without_state_change(&mut engine, 1, &answer);
    reject_without_state_change(&mut engine, 0, &submit_resolution_choice(vec![u32::MAX]));

    engine
        .apply_scenario_command(0, &answer)
        .expect("the offered counter replacement order resumes the Program");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        4,
        "both Doubling Seasons modify the one counter placement"
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "the Program tail runs once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    reject_without_state_change(&mut engine, 0, &answer);
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        4
    );
}
