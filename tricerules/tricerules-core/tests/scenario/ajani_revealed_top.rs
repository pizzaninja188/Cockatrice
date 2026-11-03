//! Prepared capability fixtures for Ajani's public top-card reveal. No actual card admission.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

const SOURCE: &str = "ajani_revealed_top_fixture";

fn game() -> (GameEngine, u32) {
    let fixture = r#"(
        id: "ajani_revealed_top_fixture", name: "Ajani Revealed Top Fixture",
        face_id: "ajani_revealed_top_fixture", mana_cost: "{0}",
        types: ["Planeswalker", "Ajani"], loyalty: 4,
        activated_abilities: [(
            ability_id: "reveal_01", presentation: Fallback, costs: [Loyalty(1)],
            effect: [RevealTopCardToHandIfMatches(
                filter: (any_of: Some([(card_type: Some(Creature)), (card_type: Some(Planeswalker))])),
                unmatched: MayPutBottom,
            ), GainLife(amount: 7, who: EachPlayer)],
        )],
    )"#;
    let both_fixture = r#"(
        id: "revealed_both_fixture", name: "Revealed Both Fixture",
        face_id: "revealed_both_fixture", mana_cost: "{0}",
        types: ["Creature", "Planeswalker", "Ajani"], power: 1, toughness: 1, loyalty: 4,
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                fixture,
                both_fixture,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            ],
            &[],
        )
        .expect("synthetic reveal profile"),
    ));
    let mut engine = GameEngine::new(
        registry,
        731_200,
        &[17, 29, 41],
        20,
        Some(vec![vec!["forest".into(); 20]; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, SOURCE);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 4);
    (engine, source)
}

fn identity(engine: &GameEngine) -> Vec<(u32, u64)> {
    engine.state.players[0]
        .library
        .iter()
        .map(|id| (*id, generation(engine, *id)))
        .collect()
}

fn generation(engine: &GameEngine, id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&id)
        .copied()
        .unwrap_or(0)
}

fn start(engine: &mut GameEngine, source: u32) -> rv1::ResolutionChoiceRequired {
    apply_ability(engine, 17, source, 0, vec![]).unwrap();
    let batch = resolve_top(engine);
    let choice = find_resolution_choice(&batch).unwrap();
    let receipt = choice.public_reveal.as_ref().unwrap();
    let reveal = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(r)) => Some(r),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        receipt, reveal,
        "published event and parked offer use one receipt"
    );
    choice
}

fn answer(choice: &rv1::ResolutionChoiceRequired, branch: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch,
            resolution_prompt_id: choice.resolution_prompt_id,
            ..Default::default()
        })),
    }
}

fn active_receipts(batch: &rv1::RuledEventBatch) -> Vec<rv1::CardsRevealed> {
    batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::ActivePublicRevealSnapshot(snapshot)) => Some(snapshot.reveals.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn rejected(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = ron::to_string(&engine.state).unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(
        ron::to_string(&engine.state).unwrap(),
        before,
        "rejection is atomic"
    );
}

fn resolve_top(engine: &mut GameEngine) -> rv1::RuledEventBatch {
    let count = engine.state.players.iter().filter(|p| !p.has_lost).count();
    let mut batch = None;
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        batch = Some(engine.apply_scenario_command(actor, &pass()).unwrap());
    }
    batch.unwrap()
}

#[test]
fn unmatched_top_offers_public_keep_or_bottom_and_parks_tail() {
    let (mut engine, source) = game();
    let top = *engine.state.players[0].library.front().unwrap();
    let generation = engine
        .state
        .zone_change_generation
        .get(&top)
        .copied()
        .unwrap_or(0);
    let life = engine.state.players[0].life;
    apply_ability(&mut engine, 17, source, 0, vec![]).unwrap();
    let batch = resolve_top(&mut engine);
    let choice = find_resolution_choice(&batch).expect("unmatched top offers Keep or Bottom");
    assert_eq!(choice.choice_kind, rv1::ChoiceKind::ResolutionBranch as i32);
    assert_eq!(choice.deciding_player_id, 17);
    assert_eq!(choice.resolution_branches.len(), 2);
    let receipt = choice
        .public_reveal
        .as_ref()
        .expect("public immutable receipt");
    assert!(!receipt.reveal_id.is_empty());
    assert_eq!(receipt.cards[0].object_id, top);
    assert_eq!(receipt.cards[0].zone_change_generation, generation);
    assert_eq!(engine.state.players[0].life, life, "tail remains parked");
}

#[test]
fn keep_and_bottom_bind_exact_revealed_incarnation_and_resume_once() {
    for count in [1, 2] {
        for branch in [0, 1] {
            let (mut engine, source) = game();
            engine.state.players[0].library.truncate(count);
            let before = identity(&engine);
            let choice = start(&mut engine, source);
            for _ in 0..2 {
                let refresh = engine.initial_response_batch();
                assert_eq!(find_resolution_choice(&refresh), Some(choice.clone()));
                assert_eq!(
                    active_receipts(&refresh),
                    vec![choice.public_reveal.clone().unwrap()]
                );
                assert_eq!(identity(&engine), before);
            }
            let command = answer(&choice, branch);
            let result = engine.apply_command(17, &command).unwrap();
            let after = identity(&engine);
            if branch == 0 {
                assert_eq!(after, before, "Keep neither reorders nor renews identity");
            } else {
                assert_eq!(after.last(), Some(&(before[0].0, before[0].1 + 1)));
                assert_eq!(&after[..count - 1], &before[1..]);
                assert!(result.events.iter().any(|event| matches!(&event.ev,Some(Ev::Log(log)) if log.text.contains("the revealed card on the bottom") && !log.text.contains("random"))));
            }
            assert!(engine.state.pending_resolution.is_none());
            assert!(active_receipts(&result).is_empty());
            assert_eq!(
                engine
                    .state
                    .players
                    .iter()
                    .map(|p| p.life)
                    .collect::<Vec<_>>(),
                vec![27; 3]
            );
            rejected(&mut engine, 17, &command);
        }
    }
}

#[test]
fn matching_creature_planeswalker_or_both_goes_to_hand_without_choice() {
    for card in ["grizzly_bears", SOURCE, "revealed_both_fixture"] {
        let (mut engine, source) = game();
        let top = inject_library_card(&mut engine, 0, card);
        engine.state.players[0].library.retain(|id| *id != top);
        engine.state.players[0].library.push_front(top);
        apply_ability(&mut engine, 17, source, 0, vec![]).unwrap();
        let result = resolve_top(&mut engine);
        assert!(find_resolution_choice(&result).is_none());
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.players[0].hand.contains(&top));
        assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
        assert_eq!(generation(&engine, top), 1);
        assert_eq!(engine.state.players[0].life, 27);
        assert!(result.events.iter().any(|e|matches!(&e.ev,Some(Ev::CardsRevealed(r)) if r.cards.len()==1 && r.cards[0].object_id==top && r.cards[0].zone_change_generation==0)));
    }
}

#[test]
fn empty_library_does_not_offer_a_choice_or_draw() {
    let (mut engine, source) = game();
    engine.state.players[0].library.clear();
    let hand = engine.state.players[0].hand.clone();
    apply_ability(&mut engine, 17, source, 0, vec![]).unwrap();
    let result = resolve_top(&mut engine);
    assert!(find_resolution_choice(&result).is_none());
    assert!(!result
        .events
        .iter()
        .any(|e| matches!(&e.ev, Some(Ev::CardsRevealed(_)))));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand, hand);
    assert!(!engine.state.players[0].has_lost);
    assert_eq!(engine.state.players[0].life, 27);
}

#[test]
fn wrong_actor_stale_identity_and_unrelated_answer_fields_are_atomic() {
    let (mut engine, source) = game();
    let choice = start(&mut engine, source);
    let valid = answer(&choice, 1);
    rejected(&mut engine, 29, &valid);
    for mutation in 0..9 {
        let mut invalid = valid.clone();
        let Some(Cmd::SubmitResolutionChoice(a)) = invalid.cmd.as_mut() else {
            unreachable!()
        };
        match mutation {
            0 => a.selected_branch_index = 2,
            1 => a.decision = rv1::ResolutionChoiceDecision::Unspecified as i32,
            2 => a.chosen_object_ids.push(1),
            3 => a.chosen_player_ids.push(29),
            4 => a.payment = Some(Default::default()),
            5 => a.restricted_mana.push(Default::default()),
            6 => a.cast_spell = Some(Default::default()),
            7 => a.spell_cast_announcement = Some(Default::default()),
            8 => a.chosen_combat_defender = Some(Default::default()),
            _ => unreachable!(),
        }
        rejected(&mut engine, 17, &invalid);
    }
    let top = choice.public_reveal.as_ref().unwrap().cards[0].object_id;
    engine.state.zone_change_generation.insert(top, 1);
    rejected(&mut engine, 17, &valid);
    engine.state.zone_change_generation.insert(top, 0);
    engine.state.players[0].library.swap(0, 1);
    rejected(&mut engine, 17, &valid);
    engine.state.players[0].library.swap(0, 1);
    engine.apply_command(17, &valid).unwrap();
    assert_eq!(engine.state.players[0].life, 27);
}

#[test]
fn source_owner_departure_preserves_prompt_and_unrelated_departure_keeps_receipt() {
    for source_owner_leaves in [false, true] {
        let (mut engine, source) = game();
        if source_owner_leaves {
            engine.state.objects.get_mut(&source).unwrap().owner = 29;
        }
        let choice = start(&mut engine, source);
        let result = engine.apply_scenario_command(29, &concede()).unwrap();
        let status = result.resolution_prompt_state.as_ref().unwrap();
        assert!(status.active);
        assert_eq!(
            status.resolution_prompt_id,
            Some(choice.resolution_prompt_id)
        );
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .resolution_prompt_id
                .get(),
            choice.resolution_prompt_id
        );
        assert_eq!(
            find_resolution_choice(&engine.initial_response_batch()),
            Some(choice.clone())
        );
        assert_eq!(
            active_receipts(&result),
            vec![choice.public_reveal.clone().unwrap()]
        );
        if source_owner_leaves {
            assert!(!engine.state.objects.contains_key(&source));
        }
        engine.apply_command(17, &answer(&choice, 0)).unwrap();
        assert_eq!(engine.state.players[0].life, 27);
        assert_eq!(engine.state.players[2].life, 27);
    }
}

#[test]
fn controller_departure_retires_choice_without_delegation_and_finishes_surviving_tail_once() {
    let (mut engine, source) = game();
    let choice = start(&mut engine, source);
    let top = choice.public_reveal.as_ref().unwrap().cards[0].object_id;
    let result = engine.apply_scenario_command(17, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert!(find_resolution_choice(&result).is_none());
    assert!(active_receipts(&result).is_empty());
    assert!(!engine.state.objects.contains_key(&top));
    assert_eq!(engine.state.players[1].life, 27);
    assert_eq!(engine.state.players[2].life, 27);
    rejected(&mut engine, 29, &answer(&choice, 1));
}

#[test]
fn departure_retires_stale_top_without_bottoming() {
    for stale_generation in [false, true] {
        let (mut engine, source) = game();
        let choice = start(&mut engine, source);
        let top = choice.public_reveal.as_ref().unwrap().cards[0].object_id;
        if stale_generation {
            engine.state.zone_change_generation.insert(top, 1);
        } else {
            engine.state.players[0].library.swap(0, 1);
        }
        let before = identity(&engine);
        engine.apply_scenario_command(29, &concede()).unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(identity(&engine), before);
        assert_eq!(engine.state.players[0].life, 27);
    }
}

#[test]
fn unadmitted_spell_program_profile_stays_closed_for_both_unmatched_policies() {
    for policy in ["KeepTop", "MayPutBottom"] {
        let fixture = format!(
            r#"(
            id: "revealed_top_program_fixture", name: "Revealed Top Program Fixture",
            face_id: "revealed_top_program_fixture", mana_cost: "{{0}}", types: ["Instant"],
            spell_program: Some((body: (steps: [
                (id: "reveal", bind: [], input: None, kind: Perform(RevealTopCardToHandIfMatches(
                    filter: (card_type: Some(Creature)), unmatched: {policy},
                ))),
            ], yields: []))),
        )"#
        );
        match tricerules_cards::CardRegistry::from_chunks_and_tokens(&[fixture.as_str()], &[]) {
            Err(tricerules_card_model::registry::RegistryError::InvalidCard { reason, .. }) => {
                assert!(
                    reason.contains("resolution-program operation whitelist"),
                    "{reason}"
                )
            }
            Err(error) => panic!("unexpected fixture error: {error}"),
            Ok(_) => panic!("unreviewed Program profile was admitted"),
        }
    }
}
