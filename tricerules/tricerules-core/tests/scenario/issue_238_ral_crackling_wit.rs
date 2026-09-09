use super::helpers::*;
use tricerules_cards::{primitives::CounterKind, CardRegistry, SpellKeyword, StaticEmblemEffect};
use tricerules_proto::ruled::v1::{ruled_event::Ev, ChoiceKind};

fn setup_ral(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "ral,_crackling_wit",
                "lightning_bolt",
                "grizzly_bears",
                "divination",
                "divination",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let ral = move_ready_to_battlefield(&mut engine, 0, "ral,_crackling_wit");
    (engine, ral)
}

#[test]
fn issue_238_ral_has_complete_oracle_presentations_and_cast_trigger_timing() {
    let definition = CardRegistry::global()
        .get("ral,_crackling_wit")
        .expect("Ral registered");
    let face = definition.primary_face();
    assert!(matches!(
        &face.triggered_abilities[0].presentation,
        tricerules_cards::AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    assert_eq!(
        face.activated_abilities
            .iter()
            .map(|ability| match &ability.presentation {
                tricerules_cards::AbilityPresentation::OracleLines(lines) => lines.clone(),
                _ => panic!("Ral loyalty abilities require Oracle mappings"),
            })
            .collect::<Vec<_>>(),
        vec![vec![2], vec![3], vec![4]]
    );

    let (mut engine, ral) = setup_ral(238_001);
    assert_eq!(
        engine.state.objects[&ral].counter_count(CounterKind::Loyalty),
        4
    );
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    grant_pool(&mut engine, 0);
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_player(1)))
        .expect("cast noncreature spell");
    assert_eq!(
        engine.state.objects[&ral].counter_count(CounterKind::Loyalty),
        4,
        "Ral's trigger must use the stack"
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&ral].counter_count(CounterKind::Loyalty),
        5,
        "the trigger resolves before the spell below it"
    );
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.card_id == "lightning_bolt" && !item.is_triggered));
    resolve_entire_stack_two_player(&mut engine);

    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let bears = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(bears, vec![]))
        .expect("cast creature spell");
    assert!(
        engine.state.stack.iter().all(|item| !item.is_triggered),
        "creature spells must not trigger Ral"
    );
}

#[test]
fn issue_238_ral_plus_one_creates_the_blue_red_prowess_otter() {
    let (mut engine, ral) = setup_ral(238_002);
    apply_ability(&mut engine, 0, ral, 0, vec![]).expect("activate +1");
    engine.apply_command(0, &pass()).expect("controller pass");
    let resolved = engine.apply_command(1, &pass()).expect("opponent pass");

    let otters = battlefield_token_oids(&engine, 0, "otter_ur_1_1_prowess");
    assert_eq!(otters.len(), 1);
    let identity = token_created_events(&resolved)[0]
        .identity
        .as_ref()
        .expect("token identity");
    assert_eq!(
        (&*identity.name, &*identity.pt, &*identity.color),
        ("Otter", "1/1", "ur")
    );
    assert_eq!(
        identity.ability_texts.len(),
        1,
        "prowess is publicly identified"
    );

    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    grant_pool(&mut engine, 0);
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_player(1)))
        .expect("cast noncreature spell");
    assert!(
        engine.state.pending_trigger_order.is_some(),
        "Ral and prowess triggers require a deterministic order"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(otters[0]), Some(2));
    assert_eq!(engine.effective_toughness(otters[0]), Some(2));
}

#[test]
fn issue_238_ral_minus_three_draws_then_privately_chooses_exactly_two_discards() {
    let (mut engine, ral) = setup_ral(238_003);
    for _ in 0..3 {
        inject_library_card(&mut engine, 0, "island");
    }
    let hand_before = engine.state.players[0].hand.len();
    let graveyard_before = engine.state.players[0].graveyard.len();
    apply_ability(&mut engine, 0, ral, 1, vec![]).expect("activate -3");
    engine.apply_command(0, &pass()).expect("controller pass");
    let batch = engine.apply_command(1, &pass()).expect("opponent pass");
    let choice = find_resolution_choice(&batch).expect("discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind, ChoiceKind::HandCards as i32);
    assert_eq!((choice.min, choice.max), (2, 2));
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);

    let discarded = choice.candidate_object_ids[..2].to_vec();
    engine
        .apply_command(0, &submit_resolution_choice(discarded.clone()))
        .expect("discard exactly two");
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(
        engine.state.players[0].graveyard.len(),
        graveyard_before + 2
    );
    assert!(discarded
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == tricerules_core::Zone::Graveyard));
}

#[test]
fn issue_238_ral_ultimate_emblem_persists_and_grants_storm() {
    let (mut engine, ral) = setup_ral(238_004);
    for _ in 0..12 {
        inject_library_card(&mut engine, 0, "island");
    }
    engine
        .state
        .objects
        .get_mut(&ral)
        .expect("Ral")
        .set_counter(CounterKind::Loyalty, 10);
    apply_ability(&mut engine, 0, ral, 2, vec![]).expect("activate -10");
    assert_eq!(
        engine.state.objects[&ral].zone,
        tricerules_core::Zone::Graveyard,
        "state-based actions remove zero-loyalty Ral before the ultimate resolves"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.static_emblems.len(), 1);
    assert!(matches!(
        engine.state.static_emblems[0].effects.as_slice(),
        [StaticEmblemEffect::GrantSpellKeyword {
            keyword: SpellKeyword::Storm,
            ..
        }]
    ));

    for ordinal in 1..=2 {
        ensure_in_hand(&mut engine, 0, "divination");
        grant_pool(&mut engine, 0);
        let divination = hand_index_for_card(&engine, 0, "divination");
        engine
            .apply_command(0, &cast_spell(divination, vec![]))
            .expect("cast Divination");
        if ordinal == 2 {
            assert_eq!(
                engine
                    .state
                    .stack
                    .iter()
                    .filter(|item| item.is_triggered)
                    .count(),
                1,
                "the persistent emblem grants storm to the later sorcery"
            );
            pass_both_players(&mut engine);
            assert_eq!(
                engine
                    .state
                    .stack
                    .iter()
                    .filter(|item| item.is_copy)
                    .count(),
                1,
                "one earlier cast produces one copy"
            );
        }
        resolve_entire_stack_two_player(&mut engine);
    }
    assert_eq!(engine.state.turn_history.current.spells_cast, 2);
    assert!(engine.initial_response_batch().events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::ZoneView(view)) if view.per_player.iter().any(|player| {
            player.player_id == 0 && player.static_emblems.len() == 1
        }))
    }));
}
