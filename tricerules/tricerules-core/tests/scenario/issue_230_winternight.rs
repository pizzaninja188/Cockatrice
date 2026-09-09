use crate::helpers::*;
use tricerules_core::Zone;

fn game() -> GameEngine {
    let mut engine = GameEngine::new(
        230,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["winternight_stories"]),
            deck_with("island", &["counterspell"]),
        ]),
        true,
    )
    .expect("Winternight Stories is fully supported");
    advance_to_main1_from_game_start(&mut engine);
    for player in &mut engine.state.players {
        for oid in std::mem::take(&mut player.hand) {
            engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Library;
            player.library.push_back(oid);
        }
    }
    engine
}

fn cast_stories(engine: &mut GameEngine) -> u32 {
    let spell = inject_card_into_hand(engine, 0, "winternight_stories");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "winternight_stories");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(engine);
    spell
}

#[test]
fn issue_230_draws_three_then_accepts_one_creature_or_any_two_cards() {
    for mode in 0..3 {
        let two = mode > 0;
        let mut engine = game();
        let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
        let land = inject_card_into_hand(&mut engine, 0, "island");
        let draws: Vec<_> = engine.state.players[0]
            .library
            .iter()
            .take(3)
            .copied()
            .collect();
        let spell = cast_stories(&mut engine);
        assert_eq!(engine.state.players[0].hand.len(), 5);
        assert!(draws
            .iter()
            .all(|oid| engine.state.players[0].hand.contains(oid)));
        let before = format!("{:?}", engine.state);
        for invalid in [vec![], vec![land], vec![creature, creature]] {
            assert!(engine
                .apply_command(0, &submit_resolution_choice(invalid))
                .is_err());
            assert_eq!(format!("{:?}", engine.state), before);
        }
        let chosen = match mode {
            0 => vec![creature],
            1 => vec![creature, land],
            _ => vec![land, draws[0]],
        };
        engine
            .apply_command(0, &submit_resolution_choice(chosen))
            .unwrap();
        assert_eq!(
            engine.state.objects[&creature].zone,
            if mode == 2 {
                Zone::Hand
            } else {
                Zone::Graveyard
            }
        );
        assert_eq!(
            engine.state.objects[&land].zone,
            if two { Zone::Graveyard } else { Zone::Hand }
        );
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn issue_230_rejects_foreign_stale_and_nonmatching_cards_atomically() {
    let mut engine = game();
    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let foreign = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    cast_stories(&mut engine);
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![creature]))
        .is_err());
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![foreign]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    *engine
        .state
        .zone_change_generation
        .entry(creature)
        .or_default() += 2;
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![creature]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn issue_230_creature_alternative_is_a_cost_and_madness_waits_for_resolution() {
    let mut engine = game();
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let wurm = inject_card_into_hand(&mut engine, 0, "arrogant_wurm");
    let spell = cast_stories(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![wurm]))
        .unwrap();
    assert!(
        engine.state.pending_resolution.is_none(),
        "Library of Leng cannot replace a cost"
    );
    assert_eq!(engine.state.objects[&wurm].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "madness trigger waits until Stories finishes"
    );
    pass_both_players(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .unwrap();
    assert_eq!(engine.state.objects[&wurm].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_230_two_card_instruction_retains_library_of_leng_replacements() {
    let mut engine = game();
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let bear = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let land = inject_card_into_hand(&mut engine, 0, "island");
    let spell = cast_stories(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![bear, land]))
        .unwrap();
    engine
        .apply_command(0, &submit_resolution_choice(vec![1]))
        .unwrap();
    assert_eq!(
        engine.state.objects[&bear].zone,
        Zone::Hand,
        "batch waits for both destinations"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![0]))
        .unwrap();
    assert_eq!(engine.state.players[0].library.front(), Some(&bear));
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_230_harmonize_reuses_reduction_and_exiles_on_resolution_or_countering() {
    use tricerules_proto::ruled::v1 as rv1;
    for tapped in [false, true] {
        for countered in [false, true] {
            let mut engine = game();
            let spell = inject_graveyard_card(&mut engine, 0, "winternight_stories");
            let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
            let discard = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    u: 1,
                    c: if tapped { 2 } else { 4 },
                    ..Default::default()
                },
            );
            let selections = if tapped {
                vec![rv1::CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    selected_object: Some(
                        rv1::cast_cost_group_selection::SelectedObject::PermanentId(bear),
                    ),
                    expected_zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&bear)
                        .copied()
                        .unwrap_or(0),
                    battlefield_objects: None,
                }]
            } else {
                vec![]
            };
            engine
                .apply_command(
                    0,
                    &RuledCommand {
                        cmd: Some(Cmd::CastSpell(rv1::CastSpell {
                            source: Some(graveyard_cast_source(
                                spell,
                                engine
                                    .state
                                    .zone_change_generation
                                    .get(&spell)
                                    .copied()
                                    .unwrap_or(0),
                            )),
                            cast_method: rv1::CastMethod::Harmonize as i32,
                            cast_cost_group_selections: selections,
                            ..Default::default()
                        })),
                    },
                )
                .unwrap();
            assert_eq!(engine.state.objects[&bear].tapped, tapped);
            assert_eq!(engine.state.players[0].mana_pool.blue, 0);
            assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
            if countered {
                engine.apply_command(0, &pass()).unwrap();
                inject_card_into_hand(&mut engine, 1, "counterspell");
                give_mana(
                    &mut engine,
                    1,
                    ManaGift {
                        u: 2,
                        ..Default::default()
                    },
                );
                let slot = hand_index_for_card(&engine, 1, "counterspell");
                engine
                    .apply_command(1, &cast_spell(slot, target_object(spell)))
                    .unwrap();
                resolve_entire_stack_two_player(&mut engine);
                assert_eq!(engine.state.players[0].hand, [discard]);
            } else {
                pass_both_players(&mut engine);
                engine
                    .apply_command(0, &submit_resolution_choice(vec![discard]))
                    .unwrap();
                assert_eq!(engine.state.players[0].hand.len(), 3);
            }
            assert_eq!(engine.state.objects[&spell].zone, Zone::Exile);
        }
    }
}

#[test]
fn issue_230_rejected_selection_does_not_change_accepted_command_replay() {
    use prost::Message;
    let run = |reject: bool| {
        let mut engine = game();
        let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
        let land = inject_card_into_hand(&mut engine, 0, "island");
        cast_stories(&mut engine);
        if reject {
            assert!(engine
                .apply_command(0, &submit_resolution_choice(vec![land]))
                .is_err());
        }
        let encoded = submit_resolution_choice(vec![creature]).encode_to_vec();
        let batch = engine
            .apply_command(0, &RuledCommand::decode(encoded.as_slice()).unwrap())
            .unwrap();
        (
            batch,
            engine.state.command_index,
            engine.state.players[0].hand.clone(),
            engine.state.players[0].graveyard.clone(),
        )
    };
    assert_eq!(run(false), run(true));
}
